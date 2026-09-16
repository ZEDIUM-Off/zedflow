use adk_graph::State;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zf_compiler::graph_compiler::GraphValidator;
use zf_execution::{
    authoring::{ConvertFlow, DeleteFlowPackage},
    commands::{Actor, Answer, CommandAuthorizer, CommandKind},
    preparation::RuntimeSelection,
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_flows::{bridge_source, composition::BridgeDefinition, flow_source, schema::Composition};
use zf_runtime::materialize::RuntimePrimitives;
use zf_storage::{
    flow_store,
    workspaces::{self, Workspace, path_id},
};

struct Policy;
#[async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        _: CommandKind,
        _: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        ensure!(actor.id == "author", "Denied");
        Ok(())
    }
}
async fn open(root: &Path) -> ExecutionService {
    for name in ["workspace", "home"] {
        std::fs::create_dir_all(root.join(name)).unwrap();
    }
    ExecutionService::open(ExecutionOptions {
        data: root.join("data"),
        workspace: root.join("workspace"),
        flow_home: root.join("home"),
        context_home: Some(root.join("home")),
        skill_dirs: vec![],
        authorizer: Arc::new(Policy),
    })
    .await
    .unwrap()
}
fn actor(service: &ExecutionService) -> Actor {
    Actor {
        id: "author".into(),
        workspace_id: service.default_workspace_id().into(),
    }
}
fn file(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn source(id: &str, wait: bool, effect: bool) -> String {
    let mut nodes = vec![node(
        "start",
        "start",
        json!({"exports":{
            "contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},
            "entries":{"main":{"node":"start","inputField":"input","outputField":"answer"}},"interactive":wait
        }}),
    )];
    if effect {
        nodes.push(node(
            "effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":format!("printf x >> {id}-effects.txt")}}),
        ));
    }
    nodes.push(if wait {
        node(
            "next",
            "input",
            json!({"field":"answer","prompt":"Continue?","responseType":"text"}),
        )
    } else {
        node("next", "set", json!({"field":"answer","value":"done"}))
    });
    nodes.push(node("end", "end", json!({})));
    let edges:Vec<_>=nodes.windows(2).map(|pair|json!({"id":format!("{}-{}",pair[0]["id"],pair[1]["id"]),"source":pair[0]["id"],"target":pair[1]["id"]})).collect();
    let doc:Composition=serde_json::from_value(json!({"formatVersion":3,"id":id,"name":id,"channels":[{"name":"answer","reducer":"overwrite"}],"nodes":nodes,"edges":edges})).unwrap();
    format!(
        "{}\n// Source conservée exactement\n",
        flow_source::render(&doc, &GraphValidator::new(&RuntimePrimitives)).unwrap()
    )
}
fn request(key: &str, hash: &str) -> StartRequest {
    StartRequest {
        definition: StartDefinition::Stored {
            key: key.into(),
            expected_hash: hash.into(),
        },
        input: State::from([("input".into(), json!("fixture"))]),
        model_bindings: json!({}),
        node_path: None,
        prepared_context: None,
        preview_metadata: None,
    }
}
async fn idle(service: &ExecutionService, id: &str) {
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(id))
        .await
        .unwrap()
        .unwrap();
}
async fn convert(
    service: &ExecutionService,
    actor: &Actor,
    path: &Path,
    source: &str,
) -> zf_execution::flow_conversion::ConversionResult {
    service
        .convert_flow(
            actor,
            ConvertFlow {
                key: path_id(path),
                expected_hash: flow_store::hash(source.as_bytes()),
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn all_legacy_roots_convert_exactly_and_admission_precedes_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path()).await;
    let actor = actor(&service);
    for (index, base) in [temp.path().join("workspace"), temp.path().join("home")]
        .into_iter()
        .enumerate()
    {
        for (namespace, relative) in [".zedflow/flows", ".agents/flows"].into_iter().enumerate() {
            let id = format!("legacy-{index}-{namespace}");
            let src = source(&id, false, false);
            let path = base.join(relative).join("nested").join(format!("{id}.rs"));
            file(&path, src.as_bytes());
            let denied = Actor {
                id: "denied".into(),
                workspace_id: actor.workspace_id.clone(),
            };
            assert!(
                service
                    .convert_flow(
                        &denied,
                        ConvertFlow {
                            key: path_id(&path),
                            expected_hash: flow_store::hash(src.as_bytes())
                        }
                    )
                    .await
                    .is_err()
            );
            assert!(path.exists());
            assert!(!base.join(".zedflow/flow").join(&id).exists());
            let result = convert(&service, &actor, &path, &src).await;
            assert!(!path.exists());
            assert_eq!(result.flow.source.as_deref(), Some(src.as_str()));
            assert_eq!(result.flow.id, id);
            assert_eq!(result.old_key, path_id(&path));
            assert_eq!(
                result.new_key,
                path_id(&base.join(".zedflow/flow").join(&id))
            );
            assert_ne!(result.old_revision, result.new_revision);
            let selection = RuntimeSelection {
                flow: result.new_key.clone(),
                entry: "main".into(),
                bridges: vec![],
                flow_hashes: BTreeMap::from([(
                    result.new_key.clone(),
                    result.new_revision.clone(),
                )]),
                bridge_hashes: BTreeMap::new(),
                contexts: BTreeMap::new(),
            };
            service.prepare(&actor, &selection).await.unwrap();
            assert!(
                service
                    .start(&actor, request(&result.old_key, &result.old_revision))
                    .await
                    .is_err()
            );
        }
    }
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn global_conversion_remaps_consumers_in_two_workspaces_and_deletion_refuses_them() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path()).await;
    let actor = actor(&service);
    let second = temp.path().join("second");
    std::fs::create_dir(&second).unwrap();
    workspaces::open(&service.database(), &second)
        .await
        .unwrap();
    let path = temp.path().join("home/.zedflow/flows/global.rs");
    let src = source("global", false, false);
    file(&path, src.as_bytes());
    let old = path_id(&path);
    let bridge = BridgeDefinition::new()
        .import("one", &old)
        .import("two", &old);
    let original = format!(
        "{}\n// référence libre : {old}\n",
        bridge_source::generate(&bridge).unwrap()
    );
    let paths: Vec<PathBuf> = [temp.path().join("workspace"), second.clone()]
        .into_iter()
        .map(|root| root.join(".zedflow/bridges/use.rs"))
        .collect();
    for path in &paths {
        file(path, original.as_bytes());
    }
    let result = convert(&service, &actor, &path, &src).await;
    assert_eq!(result.changed_consumers.len(), 2);
    for path in &paths {
        let changed = std::fs::read_to_string(path).unwrap();
        assert!(changed.contains(&format!("// référence libre : {old}")));
        assert!(
            bridge_source::parse(&changed)
                .unwrap()
                .imports
                .values()
                .all(|import| import.flow == result.new_key)
        );
    }
    assert!(
        service
            .delete_flow_package(
                &actor,
                DeleteFlowPackage {
                    key: result.new_key.clone(),
                    expected_hash: result.new_revision.clone()
                }
            )
            .await
            .is_err()
    );
    assert!(result.flow.path.exists());
    for path in paths {
        std::fs::remove_file(path).unwrap();
    }
    service
        .delete_flow_package(
            &actor,
            DeleteFlowPackage {
                key: result.new_key,
                expected_hash: result.new_revision,
            },
        )
        .await
        .unwrap();
    assert!(!result.flow.path.exists());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn conversion_and_deletion_leave_waiting_and_completed_runs_frozen_and_resumable() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path()).await;
    let actor = actor(&service);
    let mut runs = Vec::new();
    for (id, wait) in [("waiting", true), ("completed", false)] {
        let path = temp
            .path()
            .join("workspace/.zedflow/flows")
            .join(format!("{id}.rs"));
        let src = source(id, wait, true);
        file(&path, src.as_bytes());
        let started = service
            .start(
                &actor,
                request(&path_id(&path), &flow_store::hash(src.as_bytes())),
            )
            .await
            .unwrap();
        let run_id = started["id"].as_str().unwrap().to_owned();
        idle(&service, &run_id).await;
        let before = service.read(&actor, &run_id).await.unwrap();
        assert_eq!(before["status"], if wait { "waiting" } else { "completed" });
        let raw: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
            .bind(&run_id)
            .fetch_one(&service.database())
            .await
            .unwrap();
        let records: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT scope,kind,key,value_ref FROM zf_records ORDER BY scope,kind,key",
        )
        .fetch_all(&service.database())
        .await
        .unwrap();
        let result = convert(&service, &actor, &path, &src).await;
        service
            .delete_flow_package(
                &actor,
                DeleteFlowPackage {
                    key: result.new_key,
                    expected_hash: result.new_revision,
                },
            )
            .await
            .unwrap();
        let after: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
            .bind(&run_id)
            .fetch_one(&service.database())
            .await
            .unwrap();
        assert_eq!(after, raw);
        let after_records: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT scope,kind,key,value_ref FROM zf_records ORDER BY scope,kind,key",
        )
        .fetch_all(&service.database())
        .await
        .unwrap();
        assert_eq!(after_records, records);
        assert_eq!(
            std::fs::read_to_string(
                temp.path()
                    .join("workspace")
                    .join(format!("{id}-effects.txt"))
            )
            .unwrap(),
            "x"
        );
        runs.push((run_id, before, wait));
    }
    service.shutdown().await.unwrap();
    drop(service);
    let service = open(temp.path()).await;
    for (id, before, wait) in runs {
        let after = service.read(&actor, &id).await.unwrap();
        assert_eq!(after["flowSource"], before["flowSource"]);
        assert_eq!(after["flowRef"], before["flowRef"]);
        assert_eq!(after["wait"], before["wait"]);
        service.definition(&actor, &id).await.unwrap();
        if wait {
            service
                .answer(
                    &actor,
                    &id,
                    Answer {
                        wait_id: after["wait"]["id"].as_str().unwrap().into(),
                        value: json!("continue"),
                        node_path: None,
                    },
                )
                .await
                .unwrap();
            idle(&service, &id).await;
            assert_eq!(
                service.read(&actor, &id).await.unwrap()["status"],
                "completed"
            );
            assert_eq!(
                std::fs::read_to_string(temp.path().join("workspace/waiting-effects.txt")).unwrap(),
                "x"
            );
        }
    }
    service.shutdown().await.unwrap();
}
