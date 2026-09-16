//! Durable commands must precede their visibility to a concurrently owned graph.
use adk_graph::State;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};
use zf_execution::{
    commands::{
        ActivateCapability, Actor, Answer, CommandAuthorizer, CommandKind, MessageRequest,
        SelectModel,
    },
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_storage::workspaces::Workspace;

struct Policy;
#[async_trait]
impl CommandAuthorizer for Policy {
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

async fn open(root: &Path) -> (ExecutionService, Actor) {
    for folder in ["workspace", "home"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    // A FIFO blocks the fixture tool until the test releases the executor. No
    // timing assumption or network/model credential is involved.
    assert!(
        std::process::Command::new("mkfifo")
            .arg(root.join("workspace/gate"))
            .status()
            .unwrap()
            .success()
    );
    let service = ExecutionService::open(ExecutionOptions {
        data: root.join("data"),
        workspace: root.join("workspace"),
        flow_home: root.join("home"),
        context_home: Some(root.join("home")),
        skill_dirs: vec![],
        authorizer: Arc::new(Policy),
    })
    .await
    .unwrap();
    let actor = Actor {
        id: "operator".into(),
        workspace_id: service.default_workspace_id().into(),
    };
    (service, actor)
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn request(middle: Value, version: u64) -> StartRequest {
    let doc = json!({"formatVersion":version,"id":"durable","name":"Durability fixture",
        "nodes":[node("s","start",json!({})),
            node("gate","tool",json!({"tool":"exec","arguments":{"command":"printf ready > ready; cat gate"}})),
            middle,node("e","end",json!({}))],
        "edges":[{"id":"a","source":"s","target":"gate"},{"id":"b","source":"gate","target":"target"},{"id":"c","source":"target","target":"e"}]});
    StartRequest {
        definition: StartDefinition::Inline(serde_json::from_value(doc).unwrap()),
        input: State::new(),
        model_bindings: json!({}),
        node_path: None,
        prepared_context: None,
        preview_metadata: None,
    }
}
async fn gated(
    service: &ExecutionService,
    actor: &Actor,
    root: &Path,
    request: StartRequest,
) -> String {
    let id = service.start(actor, request).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    tokio::time::timeout(Duration::from_secs(15), async {
        while !root.join("workspace/ready").exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("tool reached FIFO gate");
    id
}
async fn reject_commands(service: &ExecutionService) {
    sqlx::query("CREATE TRIGGER reject_commands BEFORE INSERT ON events WHEN json_extract(NEW.document, '$.actor.id') IS NOT NULL BEGIN SELECT RAISE(ABORT, 'fixture command rejection'); END")
        .execute(&service.database()).await.unwrap();
}
fn rejected(error: anyhow::Error) {
    assert!(
        format!("{error:#}").contains("fixture command rejection"),
        "{error:#}"
    );
}
async fn finish(service: &ExecutionService, id: &str, root: &Path) {
    let gate = root.join("workspace/gate");
    tokio::task::spawn_blocking(move || std::fs::write(gate, "release\n"))
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(id))
        .await
        .expect("owner drains")
        .unwrap();
}
fn message(id: &str) -> MessageRequest {
    MessageRequest {
        id: id.into(),
        kind: "followup".into(),
        text: id.into(),
        node_path: None,
    }
}

#[tokio::test]
async fn rejected_queue_removal_and_cancel_cannot_change_running_inbox() {
    let root = tempfile::tempdir().unwrap();
    let (service, actor) = open(root.path()).await;
    let id = gated(
        &service,
        &actor,
        root.path(),
        request(node("target", "inbox", json!({"field":"input"})), 1),
    )
    .await;
    service
        .queue(&actor, &id, message("committed"))
        .await
        .unwrap();
    reject_commands(&service).await;
    rejected(
        service
            .queue(&actor, &id, message("rejected"))
            .await
            .unwrap_err(),
    );
    rejected(
        service
            .remove_message(&actor, &id, "committed")
            .await
            .unwrap_err(),
    );
    rejected(service.cancel(&actor, &id).await.unwrap_err());
    finish(&service, &id, root.path()).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(run["state"]["input"], "committed");
    assert_eq!(run["queue"].as_array().unwrap().len(), 1);
    assert_eq!(run["queue"][0]["status"], "consumed");
    assert_eq!(run["consumedMessages"], json!(["committed"]));
    assert_ne!(run["abortRequested"], true);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn rejected_new_message_is_never_delivered_to_running_inbox() {
    let root = tempfile::tempdir().unwrap();
    let (service, actor) = open(root.path()).await;
    let id = gated(
        &service,
        &actor,
        root.path(),
        request(node("target", "inbox", json!({"field":"input"})), 1),
    )
    .await;
    reject_commands(&service).await;
    rejected(
        service
            .queue(&actor, &id, message("rejected"))
            .await
            .unwrap_err(),
    );
    finish(&service, &id, root.path()).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "waiting", "{run:#}");
    assert!(run["queue"].as_array().unwrap().is_empty());
    assert!(run["consumedMessages"].as_array().is_none_or(Vec::is_empty));
    assert_ne!(run["state"]["input"], "rejected");
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn rejected_model_selection_cannot_supply_a_running_model() {
    let root = tempfile::tempdir().unwrap();
    let (service, actor) = open(root.path()).await;
    let id = gated(
        &service,
        &actor,
        root.path(),
        request(
            node("target", "agent", json!({"modelBinding":"runtime"})),
            1,
        ),
    )
    .await;
    reject_commands(&service).await;
    rejected(
        service
            .select_model(
                &actor,
                &id,
                SelectModel {
                    node_path: "target".into(),
                    selection: json!({"provider":"fixture","model":"fixture"}),
                    revision: 0,
                },
            )
            .await
            .unwrap_err(),
    );
    finish(&service, &id, root.path()).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "waiting", "{run:#}");
    assert_eq!(run["wait"]["kind"], "model_selection");
    assert!(run["modelBindings"].as_object().unwrap().is_empty());
    assert_eq!(run["modelRevision"], 0);
    rejected(
        service
            .answer(
                &actor,
                &id,
                Answer {
                    wait_id: run["wait"]["id"].as_str().unwrap().into(),
                    value: json!({"provider":"fixture","model":"fixture"}),
                    node_path: None,
                },
            )
            .await
            .unwrap_err(),
    );
    let unchanged = service.read(&actor, &id).await.unwrap();
    assert_eq!(unchanged["wait"], run["wait"]);
    assert_eq!(unchanged["modelBindings"], run["modelBindings"]);
    assert_eq!(unchanged["modelRevision"], 0);
    sqlx::query("DROP TRIGGER reject_commands")
        .execute(&service.database())
        .await
        .unwrap();
    service
        .select_model(
            &actor,
            &id,
            SelectModel {
                node_path: "target".into(),
                selection: json!({"provider":"fixture","model":"fixture"}),
                revision: 0,
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(&id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        service.read(&actor, &id).await.unwrap()["status"],
        "completed"
    );

    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn rejected_activation_cannot_enter_a_running_agent_context() {
    let root = tempfile::tempdir().unwrap();
    let (service, actor) = open(root.path()).await;
    let config = json!({"provider":"fixture","model":"fixture","attachments":{"instructions":{"items":[{"id":"secret","activation":"explicit","source":{"kind":"text","text":"REJECTED-CAPABILITY"}}]}}});
    let id = gated(
        &service,
        &actor,
        root.path(),
        request(node("target", "agent", config), 2),
    )
    .await;
    reject_commands(&service).await;
    rejected(
        service
            .activate_capability(
                &actor,
                &id,
                ActivateCapability {
                    node_path: "target".into(),
                    item_id: "secret".into(),
                    skill_name: None,
                    active: true,
                },
            )
            .await
            .unwrap_err(),
    );
    finish(&service, &id, root.path()).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "completed", "{run:#}");
    assert!(run["capabilityActivations"]["target"].is_null());
    let content = zf_storage::content_store::ContentStore::new(service.database())
        .await
        .unwrap();
    let captures = content
        .records_of_kind(&id, "capability-snapshots")
        .await
        .unwrap();
    assert!(!captures.is_empty());
    for capture in captures {
        let value = content.resolve(&capture.value_ref).await.unwrap();
        assert!(
            !value.to_string().contains("REJECTED-CAPABILITY"),
            "{value:#}"
        );
    }
    service.shutdown().await.unwrap();
}
