use adk_graph::State;
use anyhow::Result;
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
    commands::{Actor, CommandAuthorizer, CommandKind},
    preparation::RuntimeSelection,
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_flows::{flow_source, schema::Composition};
use zf_runtime::materialize::RuntimePrimitives;
use zf_storage::{
    flow_packages,
    workspaces::{Workspace, path_id},
};

struct FixtureAuthority;
#[async_trait]
impl CommandAuthorizer for FixtureAuthority {
    async fn authorize(
        &self,
        _: &Actor,
        _: CommandKind,
        _: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        Ok(())
    }
}
async fn open(root: &Path) -> ExecutionService {
    for dir in ["workspace", "home"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    ExecutionService::open(ExecutionOptions {
        data: root.join("data"),
        workspace: root.join("workspace"),
        flow_home: root.join("home"),
        context_home: Some(root.join("home")),
        skill_dirs: vec![],
        authorizer: Arc::new(FixtureAuthority),
    })
    .await
    .unwrap()
}
fn package(root: &Path) -> PathBuf {
    let path = root.join("workspace/.zedflow/flow/sample");
    std::fs::create_dir_all(&path).unwrap();
    let doc:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"sample","name":"Sample",
        "nodes":[{"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":{
            "contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},
            "entries":{"main":{"node":"start","inputField":"input","outputField":"output"}},"interactive":false}}}},
        {"id":"action","position":{"x":100,"y":0},"data":{"kind":"set","label":"Set","config":{"field":"output","value":"done"}}},
        {"id":"end","position":{"x":200,"y":0},"data":{"kind":"end","label":"End","config":{}}}],
        "edges":[{"id":"a","source":"start","target":"action"},{"id":"b","source":"action","target":"end"}]})).unwrap();
    std::fs::write(
        path.join("flow.rs"),
        flow_source::render(&doc, &GraphValidator::new(&RuntimePrimitives)).unwrap(),
    )
    .unwrap();
    std::fs::write(path.join("flow.json"),serde_json::to_vec(&json!({"formatVersion":1,"id":"sample","name":"Sample","entry":"flow.rs","files":["flow.rs","README.md"]})).unwrap()).unwrap();
    std::fs::write(path.join("README.md"), "first revision").unwrap();
    path
}
fn request(path: &Path, revision: &str) -> StartRequest {
    StartRequest {
        definition: StartDefinition::Stored {
            key: path_id(path),
            expected_hash: revision.into(),
        },
        input: State::from([("input".into(), json!("fixture"))]),
        model_bindings: json!({}),
        node_path: None,
        prepared_context: None,
        preview_metadata: None,
    }
}
#[tokio::test]
async fn stored_run_persists_exact_package_and_rejects_a_stale_secondary_file_revision() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path()).await;
    let actor = Actor {
        id: "fixture".into(),
        workspace_id: service.default_workspace_id().into(),
    };
    let path = package(temp.path());
    let captured = flow_packages::capture(&path).await.unwrap();
    std::fs::write(path.join("README.md"), "second revision").unwrap();
    assert!(
        service
            .start(&actor, request(&path, &captured.root))
            .await
            .is_err()
    );
    std::fs::write(path.join("README.md"), "first revision").unwrap();
    let ack = service
        .start(&actor, request(&path, &captured.root))
        .await
        .unwrap();
    let id = ack["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(id))
        .await
        .unwrap()
        .unwrap();
    let run = service.read(&actor, id).await.unwrap();
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(run["flowPackage"], serde_json::to_value(&captured).unwrap());
    assert_ne!(run["flowRef"]["hash"], run["executedSourceHash"]);
    let stored: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
        .bind(id)
        .fetch_one(&service.database())
        .await
        .unwrap();
    let stored: Value = serde_json::from_str(&stored).unwrap();
    assert!(stored.get("flowPackage").is_none());
    assert!(stored["flowPackageRef"].is_string());
    let detail = service.definition(&actor, id).await.unwrap();
    assert_eq!(
        detail["flowPackage"],
        serde_json::to_value(&captured).unwrap()
    );
    service.shutdown().await.unwrap();
    std::fs::write(path.join("README.md"), "third revision").unwrap();
    let reopened = open(temp.path()).await;
    let restored = reopened.read(&actor, id).await.unwrap();
    assert_eq!(
        restored["flowPackage"],
        serde_json::to_value(&captured).unwrap()
    );
    assert_eq!(restored["flowSource"], run["flowSource"]);
    reopened.shutdown().await.unwrap();
}
#[tokio::test]
async fn composition_preparation_pins_package_revision_without_confusing_the_rust_hash() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path()).await;
    let actor = Actor {
        id: "fixture".into(),
        workspace_id: service.default_workspace_id().into(),
    };
    let path = package(temp.path());
    let captured = flow_packages::capture(&path).await.unwrap();
    let key = path_id(&path);
    let selection = RuntimeSelection {
        flow: key.clone(),
        entry: "main".into(),
        bridges: vec![],
        flow_hashes: BTreeMap::from([(key.clone(), captured.root.clone())]),
        bridge_hashes: BTreeMap::new(),
        contexts: BTreeMap::new(),
    };
    let prepared = service.prepare(&actor, &selection).await.unwrap();
    assert_eq!(prepared.definitions.flow_packages[&key], captured);
    assert_ne!(prepared.definitions.flow_hashes[&key], captured.root);
    std::fs::write(path.join("README.md"), "changed").unwrap();
    assert!(service.prepare(&actor, &selection).await.is_err());
    prepared.validate(&RuntimePrimitives).unwrap();
    service.shutdown().await.unwrap();
}
