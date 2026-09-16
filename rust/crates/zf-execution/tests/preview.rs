use adk_graph::State;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use zf_context::context::{
    ContextBlock, ContextExpr, ContextStrategy, FragmentFormat, FragmentRole,
};
use zf_core::types::DataType;
use zf_execution::{
    commands::{Actor, Answer, CommandAuthorizer, CommandKind, ExecutionError},
    preview::PreviewRun,
    service::{ExecutionOptions, ExecutionService},
};
use zf_flows::schema::Composition;
use zf_storage::{
    context_store::ContextStore,
    workspaces::{self, Workspace},
};

struct Policy {
    origin: PathBuf,
    calls: Mutex<Vec<(CommandKind, String)>>,
}
#[async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        kind: CommandKind,
        workspace: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push((kind, workspace.id.clone()));
        ensure!(
            actor.id == "operator",
            ExecutionError::Forbidden("Caller denied".into())
        );
        if kind == CommandKind::Start {
            ensure!(
                workspace.path == self.origin,
                ExecutionError::Forbidden("Start only allowed in origin".into())
            );
        }
        Ok(())
    }
}
async fn open(root: &Path, policy: Arc<Policy>) -> ExecutionService {
    ExecutionService::open(ExecutionOptions {
        data: root.join("data"),
        workspace: root.join("workspace"),
        flow_home: root.join("home"),
        context_home: Some(root.join("home")),
        skill_dirs: vec![],
        authorizer: policy,
    })
    .await
    .unwrap()
}
async fn fixture(root: &Path) -> (ExecutionService, Arc<Policy>, Actor) {
    for directory in ["workspace", "home/.pi/agent"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    let policy = Arc::new(Policy {
        origin: root.join("workspace"),
        calls: Mutex::new(vec![]),
    });
    let service = open(root, policy.clone()).await;
    let actor = Actor {
        id: "operator".into(),
        workspace_id: service.default_workspace_id().into(),
    };
    (service, policy, actor)
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn doc(nodes: Vec<Value>) -> Composition {
    let edges: Vec<_> = nodes
        .windows(2)
        .enumerate()
        .map(
            |(i, pair)| json!({"id":format!("e{i}"),"source":pair[0]["id"],"target":pair[1]["id"]}),
        )
        .collect();
    serde_json::from_value(
        json!({"formatVersion":3,"id":"preview","name":"Preview","nodes":nodes,"edges":edges}),
    )
    .unwrap()
}
fn request(composition: Composition) -> PreviewRun {
    PreviewRun {
        composition,
        input: State::new(),
        model_bindings: json!({}),
    }
}
fn strategy(text: &str) -> ContextStrategy {
    ContextStrategy::new("draft", "Draft").with_program(vec![ContextBlock::emit(
        "instruction",
        FragmentRole::Instruction,
        FragmentFormat::Text,
        ContextExpr::literal(DataType::Text, json!(text)),
    )])
}
async fn idle(service: &ExecutionService, id: &str) {
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(id))
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn denied_or_maintenance_preview_has_no_capture_workspace_or_execution_effects() {
    let root = tempfile::tempdir().unwrap();
    let (service, policy, actor) = fixture(root.path()).await;
    let composition = doc(vec![
        node("s", "start", json!({})),
        node(
            "agent",
            "agent",
            json!({"provider":"fixture","contextStrategy":"missing"}),
        ),
        node("e", "end", json!({})),
    ]);
    let denied = Actor {
        id: "intruder".into(),
        workspace_id: actor.workspace_id.clone(),
    };
    let error = service
        .preview(&denied, request(composition.clone()))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<ExecutionError>(),
            Some(ExecutionError::Forbidden(_))
        ),
        "{error:#}"
    );
    let maintenance = service.try_begin_maintenance().unwrap();
    let error = service
        .preview(&actor, request(composition))
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    drop(maintenance);
    assert!(!root.path().join("data/previews").exists());
    assert_eq!(
        workspaces::list(&service.database()).await.unwrap().len(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM runs")
            .fetch_one(&service.database())
            .await
            .unwrap(),
        0
    );
    assert_eq!(policy.calls.lock().unwrap().len(), 1);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn preview_keeps_origin_admission_nested_programs_context_and_history_after_source_edits() {
    let root = tempfile::tempdir().unwrap();
    let (service, policy, actor) = fixture(root.path()).await;
    let origin = root.path().join("workspace");
    let home_instructions = root.path().join("home/.pi/agent/AGENTS.md");
    std::fs::write(origin.join("AGENTS.md"), "workspace captured at preview").unwrap();
    std::fs::write(&home_instructions, "home captured at preview").unwrap();
    let sources = ContextStore::new(origin.clone());
    let saved = sources
        .save(&strategy("captured strategy"), None)
        .await
        .unwrap();
    let child = doc(vec![
        node("s", "start", json!({})),
        node(
            "agent",
            "agent",
            json!({"provider":"fixture","fixtureSteps":[{"echoRequest":true}],"contextStrategy":{"key":"draft","hash":saved.hash},"contextBindings":{}}),
        ),
        node("e", "end", json!({})),
    ]);
    let composition = doc(vec![
        node("s", "start", json!({})),
        node(
            "wait",
            "input",
            json!({"field":"input","prompt":"Continue?","responseType":"text"}),
        ),
        node(
            "effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":"printf preview > result.txt"}}),
        ),
        node("child", "subgraph", json!({"composition":child})),
        node("e", "end", json!({})),
    ]);
    let ack = service
        .preview(&actor, request(composition.clone()))
        .await
        .unwrap();
    assert_eq!(
        *policy.calls.lock().unwrap(),
        vec![(CommandKind::Start, actor.workspace_id.clone())]
    );
    let id = ack["id"].as_str().unwrap();
    let target_actor = Actor {
        id: actor.id.clone(),
        workspace_id: ack["workspaceId"].as_str().unwrap().into(),
    };
    assert_ne!(actor.workspace_id, target_actor.workspace_id);
    idle(&service, id).await;
    let waiting = service.read(&target_actor, id).await.unwrap();
    assert_eq!(waiting["status"], "waiting", "{waiting:#}");
    assert_eq!(
        waiting["startedBy"],
        json!({"id":"operator","workspaceId":actor.workspace_id})
    );
    let workspace = workspaces::get(&service.database(), &target_actor.workspace_id)
        .await
        .unwrap();
    assert!(!workspace.open);
    assert!(
        workspace
            .path
            .starts_with(root.path().join("data/previews"))
    );
    assert_eq!(waiting["context"]["cwd"], json!(origin));
    let instructions = waiting["context"]["instructions"].as_array().unwrap();
    for text in ["workspace captured at preview", "home captured at preview"] {
        assert!(instructions.iter().any(|i| i["content"] == text));
    }
    let frozen = &waiting["composition"]["nodes"][3]["data"]["config"]["composition"]["nodes"][1]["data"]
        ["config"];
    assert_eq!(frozen["contextProgram"]["hash"], saved.hash);
    assert!(frozen.get("contextStrategy").is_none());
    assert!(
        frozen["contextProgram"]["source"]
            .as_str()
            .unwrap()
            .contains("captured strategy")
    );
    assert_eq!(waiting["preview"]["sourceWorkspaceId"], actor.workspace_id);
    assert_eq!(waiting["preview"]["sourceWorkspacePath"], json!(origin));
    assert_eq!(waiting["preview"]["temporaryWorkspace"], true);
    let source_ref = waiting["preview"]["sourceCompositionRef"].as_str().unwrap();
    assert_eq!(
        service.content().resolve(source_ref).await.unwrap(),
        json!(composition)
    );
    assert!(service.read(&actor, id).await.is_err());

    sources
        .save(&strategy("edited after preview"), Some(&saved.hash))
        .await
        .unwrap();
    std::fs::write(origin.join("AGENTS.md"), "workspace edited after preview").unwrap();
    std::fs::write(home_instructions, "home edited after preview").unwrap();
    service
        .answer(
            &target_actor,
            id,
            Answer {
                node_path: None,
                wait_id: waiting["wait"]["id"].as_str().unwrap().into(),
                value: json!("continue"),
            },
        )
        .await
        .unwrap();
    idle(&service, id).await;
    let completed = service.read(&target_actor, id).await.unwrap();
    assert_eq!(completed["status"], "completed", "{completed:#}");
    assert_eq!(completed["context"], waiting["context"]);
    assert_eq!(completed["composition"], waiting["composition"]);
    assert!(!origin.join("result.txt").exists());
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("result.txt")).unwrap(),
        "preview"
    );
    assert!(!workspace.path.join(".zedflow/context/draft.rs").exists());
    service.shutdown().await.unwrap();
    let reopened = open(root.path(), policy).await;
    let definition = reopened.definition(&target_actor, id).await.unwrap();
    assert_eq!(definition["composition"], completed["composition"]);
    assert_eq!(definition["preview"], completed["preview"]);
    assert_eq!(
        reopened.content().resolve(source_ref).await.unwrap(),
        json!(composition)
    );
    reopened.shutdown().await.unwrap();
}

#[test]
fn preview_body_cannot_supply_trusted_context_or_workspace_identity() {
    let composition = doc(vec![
        node("s", "start", json!({})),
        node("e", "end", json!({})),
    ]);
    let value = json!({"composition":composition});
    let decoded: PreviewRun = serde_json::from_value(value.clone()).unwrap();
    assert!(decoded.input.is_empty());
    assert_eq!(decoded.model_bindings, json!({}));
    for field in ["preparedContext", "previewMetadata", "workspaceId"] {
        let mut invalid = value.clone();
        invalid[field] = json!({});
        assert!(
            serde_json::from_value::<PreviewRun>(invalid).is_err(),
            "{field}"
        );
    }
}
