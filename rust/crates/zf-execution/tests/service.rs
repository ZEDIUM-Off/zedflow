use adk_graph::State;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use zf_execution::{
    commands::{Actor, Answer, CommandAuthorizer, CommandKind, ExecutionError, RenameRun},
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_flows::schema::Composition;
use zf_storage::workspaces::Workspace;

#[derive(Default)]
struct Policy {
    calls: Mutex<Vec<CommandKind>>,
}
#[async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        kind: CommandKind,
        _: &Workspace,
        _: Option<&Value>,
    ) -> Result<()> {
        ensure!(
            actor.id == "operator",
            ExecutionError::Forbidden("Caller is not the local operator".into())
        );
        self.calls.lock().unwrap().push(kind);
        Ok(())
    }
}
async fn open(root: &Path, policy: Arc<Policy>) -> ExecutionService {
    for folder in ["workspace", "home"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
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
fn actor(service: &ExecutionService) -> Actor {
    Actor {
        id: "operator".into(),
        workspace_id: service.default_workspace_id().into(),
    }
}
fn flow(wait: bool) -> Composition {
    let middle = if wait {
        json!({"id":"wait","position":{"x":0,"y":0},"data":{"kind":"input","label":"Wait","config":{"field":"answer","prompt":"Continue?","responseType":"text"}}})
    } else {
        json!({"id":"wait","position":{"x":0,"y":0},"data":{"kind":"set","label":"Set","config":{"field":"answer","value":"done"}}})
    };
    serde_json::from_value(json!({"id":"fixture","name":"Fixture","nodes":[{"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},middle,{"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}],"edges":[{"id":"a","source":"s","target":"wait"},{"id":"b","source":"wait","target":"e"}]})).unwrap()
}
fn request(wait: bool) -> StartRequest {
    StartRequest {
        definition: StartDefinition::Inline(flow(wait)),
        input: State::new(),
        model_bindings: json!({}),
        node_path: None,
        prepared_context: None,
        preview_metadata: None,
    }
}
async fn idle(service: &ExecutionService, id: &str) {
    tokio::time::timeout(Duration::from_secs(15), service.wait_idle(id))
        .await
        .expect("execution drains")
        .unwrap();
}

#[tokio::test]
async fn admission_and_maintenance_apply_without_http_and_shutdown_closes_admission() {
    let temp = tempfile::tempdir().unwrap();
    let policy = Arc::new(Policy::default());
    let service = open(temp.path(), policy.clone()).await;
    let actor = actor(&service);
    let denied = Actor {
        id: "untrusted".into(),
        workspace_id: actor.workspace_id.clone(),
    };
    assert!(matches!(
        service
            .start(&denied, request(false))
            .await
            .unwrap_err()
            .downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Forbidden(_))
    ));
    let maintenance = service.try_begin_maintenance().unwrap();
    assert!(matches!(
        service
            .start(&actor, request(false))
            .await
            .unwrap_err()
            .downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    drop(maintenance);
    let ack = service.start(&actor, request(false)).await.unwrap();
    let id = ack["id"].as_str().unwrap();
    // No sleep/yield between start and wait_idle: an unpolled task is still owned.
    idle(&service, id).await;
    let run = service.read(&actor, id).await.unwrap();
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(run["state"]["answer"], "done");
    assert_eq!(run["interactive"], false);
    assert_eq!(run["startedBy"]["id"], "operator");
    service.shutdown().await.unwrap();
    assert!(matches!(
        service
            .start(&actor, request(false))
            .await
            .unwrap_err()
            .downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    assert!(policy.calls.lock().unwrap().contains(&CommandKind::Start));
}

#[tokio::test]
async fn two_runs_keep_waits_distinct_and_duplicate_answers_have_one_winner() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let (a, b) = tokio::join!(
        service.start(&actor, request(true)),
        service.start(&actor, request(true))
    );
    let a = a.unwrap()["id"].as_str().unwrap().to_owned();
    let b = b.unwrap()["id"].as_str().unwrap().to_owned();
    idle(&service, &a).await;
    idle(&service, &b).await;
    let before = service.read(&actor, &a).await.unwrap();
    assert_eq!(before["status"], "waiting", "{before:#}");
    assert_eq!(before["interactive"], true);
    let wait = before["wait"]["id"].as_str().unwrap().to_owned();
    let wrong = service
        .answer(
            &actor,
            &b,
            Answer {
                wait_id: wait.clone(),
                value: json!("wrong run"),
                node_path: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        wrong.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Conflict(_))
    ));
    let response = || Answer {
        wait_id: wait.clone(),
        value: json!("accepted"),
        node_path: None,
    };
    let (first, second) = tokio::join!(
        service.answer(&actor, &a, response()),
        service.answer(&actor, &a, response())
    );
    assert_ne!(first.is_ok(), second.is_ok());
    idle(&service, &a).await;
    let run = service.read(&actor, &a).await.unwrap();
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(
        run["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["id"] == wait)
            .count(),
        1
    );
    assert_eq!(service.read(&actor, &b).await.unwrap()["status"], "waiting");
    service.cancel(&actor, &b).await.unwrap();
    assert_eq!(service.read(&actor, &b).await.unwrap()["status"], "stopped");
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn workspace_scope_and_actor_are_checked_before_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let id = service.start(&actor, request(true)).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let other = temp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let other = zf_storage::workspaces::open(&service.database(), &other)
        .await
        .unwrap();
    let foreign = Actor {
        id: "operator".into(),
        workspace_id: other.id,
    };
    assert!(matches!(
        service
            .rename(
                &foreign,
                &id,
                RenameRun {
                    name: "foreign".into()
                }
            )
            .await
            .unwrap_err()
            .downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Forbidden(_))
    ));
    service
        .rename(
            &actor,
            &id,
            RenameRun {
                name: "Renamed".into(),
            },
        )
        .await
        .unwrap();
    let event: String =
        sqlx::query_scalar("SELECT document FROM events WHERE run=? ORDER BY seq DESC LIMIT 1")
            .bind(&id)
            .fetch_one(&service.database())
            .await
            .unwrap();
    let event: Value = serde_json::from_str(&event).unwrap();
    assert_eq!(
        event["actor"],
        json!({"id":actor.id,"workspaceId":actor.workspace_id})
    );
    assert_eq!(service.read(&actor, &id).await.unwrap()["name"], "Renamed");
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_retains_wait_and_resumes_exact_checkpoint_without_replaying_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let mut start = request(true);
    let mut doc = serde_json::to_value(flow(true)).unwrap();
    doc["nodes"].as_array_mut().unwrap().push(json!({"id":"effect","position":{"x":0,"y":0},"data":{"kind":"tool","label":"Effect","config":{"tool":"exec","arguments":{"command":"printf x >> effect-count.txt"}}}}));
    doc["edges"] = json!([{"id":"a","source":"s","target":"effect"},{"id":"b","source":"effect","target":"wait"},{"id":"c","source":"wait","target":"e"}]);
    start.definition = StartDefinition::Inline(serde_json::from_value(doc).unwrap());
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/effect-count.txt")).unwrap(),
        "x"
    );
    let before = service.read(&actor, &id).await.unwrap();
    let wait = before["wait"].clone();
    let source = before["flowSource"].clone();
    service.shutdown().await.unwrap();
    drop(service);
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let restored = service.read(&actor, &id).await.unwrap();
    assert_eq!(restored["wait"], wait);
    assert_eq!(restored["checkpoint"], before["checkpoint"]);
    assert_eq!(restored["flowSource"], source);
    service
        .answer(
            &actor,
            &id,
            Answer {
                wait_id: wait["id"].as_str().unwrap().into(),
                value: json!("after restart"),
                node_path: None,
            },
        )
        .await
        .unwrap();
    idle(&service, &id).await;
    let after = service.read(&actor, &id).await.unwrap();
    assert_eq!(after["status"], "completed", "{after:#}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/effect-count.txt")).unwrap(),
        "x"
    );
    assert_eq!(
        after["activities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["node"] == "s")
            .count(),
        before["activities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["node"] == "s")
            .count()
    );
    service.shutdown().await.unwrap();
}

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn edge(a: &str, b: &str) -> Value {
    json!({"id":format!("{a}-{b}"),"source":a,"target":b})
}
fn docs(mode: &str, wait: bool) -> (Composition, Composition, Value) {
    let root_contract = json!({"entries":{"main":{"input":{"kind":"text"}}},"branches":{"delegate":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["node","condition"]}},"data":{"notes":{"dataType":{"kind":"text"},"permissions":{"read":true,"write":true}}}});
    let worker_contract = json!({"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}},"requires":{"notes":{"dataType":{"kind":"text"},"permissions":{"read":true}}},"data":{"answer":{"dataType":{"kind":"text"},"permissions":{"read":true,"write":true}}}});
    let root_exports = json!({"contract":root_contract,"entries":{"main":{"node":"route","inputField":"input"}},"branches":{"delegate":"route"},"data":{"notes":"notes"},"requires":{},"interactive":false});
    let worker_exports = json!({"contract":worker_contract,"entries":{"main":{"node":"effect","inputField":"input","outputField":"output"}},"branches":{},"data":{"answer":"output"},"requires":{"notes":"notes"},"interactive":wait});
    let root:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"root-flow","name":"Root","channels":[{"name":"notes","reducer":"overwrite","default":"seed"}],"nodes":[node("s","start",json!({"exports":root_exports})),node("route","route",json!({"branch":"delegate"})),node("parent-effect","tool",json!({"tool":"exec","arguments":{"command":"printf p >> parent-effects.txt"}})),node("e","end",json!({}))],"edges":[edge("s","route"),edge("route","parent-effect"),edge("parent-effect","e")]})).unwrap();
    let mut nodes = vec![
        node("s", "start", json!({"exports":worker_exports})),
        node(
            "effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":"printf x >> route-effects.txt"}}),
        ),
    ];
    let mut edges = vec![edge("s", "effect")];
    if wait {
        nodes.push(node(
            "gate",
            "input",
            json!({"prompt":"Child question","field":"input"}),
        ));
        edges.push(edge("effect", "gate"));
        edges.push(edge("gate", "out"));
    } else {
        edges.push(edge("effect", "out"));
    }
    nodes.extend([
        node("out", "set", json!({"field":"output","value":"{{notes}}"})),
        node("e", "end", json!({})),
    ]);
    edges.push(edge("out", "e"));
    let worker:Composition=serde_json::from_value(json!({"formatVersion":3,"id":"worker-flow","name":"Worker","channels":[{"name":"notes","reducer":"overwrite"}],"nodes":nodes,"edges":edges})).unwrap();
    let bridge = json!({"imports":{"worker":{"flow":"worker-flow"}},"connections":{"call":{"from":{"instance":"root","port":"delegate"},"to":{"instance":"worker","port":"main"},"invocation":"node","mode":mode}},"bindings":{"notes":{"from":{"instance":"root","port":"notes"},"to":{"instance":"worker","port":"notes"},"permissions":{"read":true}}}});
    (root, worker, bridge)
}

async fn routed_request(
    root: &Path,
    service: &ExecutionService,
    mode: &str,
    wait: bool,
) -> StartRequest {
    let (main, worker, mut bridge) = docs(mode, wait);
    let workspace =
        zf_storage::workspaces::get(&service.database(), service.default_workspace_id())
            .await
            .unwrap();
    let directory = root.join("workspace/.zedflow/flows");
    std::fs::create_dir_all(&directory).unwrap();
    let mut keys = Vec::new();
    for doc in [&main, &worker] {
        let path = directory.join(format!("{}.rs", doc.id));
        let bytes = zf_flows::flow_format::render(
            doc,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap();
        std::fs::write(&path, bytes).unwrap();
        keys.push(zf_storage::workspaces::path_id(&path));
    }
    bridge["imports"]["worker"]["flow"] = json!(keys[1]);
    zf_storage::bridge_store::BridgeStore::new(workspace.path)
        .unwrap()
        .save("bridge", &serde_json::from_value(bridge).unwrap(), None)
        .await
        .unwrap();
    StartRequest {
        definition: StartDefinition::Composition(zf_execution::preparation::RuntimeSelection {
            flow: keys.remove(0),
            entry: "main".into(),
            bridges: vec!["bridge".into()],
            flow_hashes: Default::default(),
            bridge_hashes: Default::default(),
            contexts: Default::default(),
        }),
        input: State::from([("input".into(), json!("task"))]),
        model_bindings: json!({}),
        node_path: None,
        prepared_context: None,
        preview_metadata: None,
    }
}
#[tokio::test]
async fn parent_child_wait_survives_restart_and_neither_effect_is_repeated() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let start = routed_request(temp.path(), &service, "callAwait", true).await;
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "waiting", "{run:#}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/route-effects.txt")).unwrap(),
        "x"
    );
    assert!(!temp.path().join("workspace/parent-effects.txt").exists());
    let wait = run["wait"].clone();
    service.shutdown().await.unwrap();
    drop(service);
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    service
        .answer(
            &actor,
            &id,
            Answer {
                wait_id: wait["id"].as_str().unwrap().into(),
                value: json!("approved"),
                node_path: None,
            },
        )
        .await
        .unwrap();
    idle(&service, &id).await;
    let result = service.read(&actor, &id).await.unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/route-effects.txt")).unwrap(),
        "x"
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/parent-effects.txt")).unwrap(),
        "p"
    );
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn launch_owner_drains_children_before_reporting_idle() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let start = routed_request(temp.path(), &service, "launch", false).await;
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let result = service.read(&actor, &id).await.unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/route-effects.txt")).unwrap(),
        "x"
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/parent-effects.txt")).unwrap(),
        "p"
    );
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn queued_input_is_distinct_from_an_answer_and_consumed_once() {
    use zf_execution::commands::MessageRequest;
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let mut doc = serde_json::to_value(flow(true)).unwrap();
    doc["nodes"][1]["data"]["kind"] = json!("inbox");
    let mut start = request(true);
    start.definition = StartDefinition::Inline(serde_json::from_value(doc).unwrap());
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let old_wait = service.read(&actor, &id).await.unwrap()["wait"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    service
        .queue(
            &actor,
            &id,
            MessageRequest {
                id: "next-message".into(),
                kind: "followup".into(),
                text: "continue".into(),
                node_path: None,
            },
        )
        .await
        .unwrap();
    idle(&service, &id).await;
    let result = service.read(&actor, &id).await.unwrap();
    assert_eq!(result["status"], "completed", "{result:#}");
    assert_eq!(result["state"]["answer"], "continue");
    assert_eq!(
        result["queue"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["id"] == "next-message" && m["status"] == "consumed")
            .count(),
        1,
        "{result:#}"
    );
    assert!(
        service
            .answer(
                &actor,
                &id,
                Answer {
                    wait_id: old_wait,
                    value: json!("stale"),
                    node_path: None
                }
            )
            .await
            .is_err()
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn observation_write_failure_joins_launched_children_before_maintenance() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let start = routed_request(temp.path(), &service, "launch", false).await;
    let (mut main, mut child, _) = docs("launch", false);
    main.nodes
        .iter_mut()
        .find(|n| n.id == "parent-effect")
        .unwrap()
        .data
        .config["arguments"]["command"] =
        json!("while [ ! -f fail-ready ]; do sleep 0.01; done; printf p >> parent-effects.txt");
    child
        .nodes
        .iter_mut()
        .find(|n| n.id == "effect")
        .unwrap()
        .data
        .config["arguments"]["command"] = json!(
        "printf '%s' $$ > child-pid; while [ ! -f release-child ]; do sleep 0.01; done; printf y > child-finished"
    );
    for doc in [&main, &child] {
        let source = zf_flows::flow_format::render(
            doc,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap();
        std::fs::write(
            temp.path()
                .join(format!("workspace/.zedflow/flows/{}.rs", doc.id)),
            source,
        )
        .unwrap();
    }
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pid_path = temp.path().join("workspace/child-pid");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !pid_path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("child reached its gated effect");
    assert!(
        service.try_begin_maintenance().is_err(),
        "the active child still owns its lease"
    );
    sqlx::query("CREATE TRIGGER reject_parent_observation BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.type')='node_activity' AND json_extract(NEW.document,'$.status')='completed' AND json_extract(NEW.document,'$.path')='root/parent-effect' BEGIN SELECT RAISE(ABORT,'fixture observation failure'); END").execute(&service.database()).await.unwrap();
    std::fs::write(temp.path().join("workspace/fail-ready"), "").unwrap();
    idle(&service, &id).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["status"], "error", "{run:#}");
    assert!(
        run["error"]
            .as_str()
            .unwrap()
            .contains("fixture observation failure")
    );
    let pid = std::fs::read_to_string(&pid_path).unwrap();
    assert!(
        !Path::new("/proc").join(pid).exists(),
        "the owned child shell must have terminated before idle"
    );
    assert!(!temp.path().join("workspace/child-finished").exists());
    let maintenance = service
        .try_begin_maintenance()
        .expect("all admitted owner tasks have settled");
    drop(maintenance);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn resumed_checkpoint_failure_is_visible_instead_of_stuck_running() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let id = service.start(&actor, request(true)).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let run = service.read(&actor, &id).await.unwrap();
    let wait = run["wait"]["id"].as_str().unwrap().to_owned();
    sqlx::query("CREATE TRIGGER reject_new_checkpoint BEFORE INSERT ON zf_checkpoints BEGIN SELECT RAISE(ABORT,'fixture checkpoint failure'); END").execute(&service.database()).await.unwrap();
    service
        .answer(
            &actor,
            &id,
            Answer {
                wait_id: wait,
                value: json!("new input"),
                node_path: None,
            },
        )
        .await
        .unwrap();
    idle(&service, &id).await;
    let failed = service.read(&actor, &id).await.unwrap();
    assert_eq!(failed["status"], "error", "{failed:#}");
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .contains("fixture checkpoint failure")
    );
    assert_eq!(failed["checkpoint"], run["checkpoint"]);
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn composed_root_inbox_uses_qualified_node_contract_to_wake() {
    use zf_execution::commands::MessageRequest;
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let mut doc = serde_json::to_value(flow(true)).unwrap();
    doc["formatVersion"] = json!(3);
    doc["nodes"][1]["data"]["kind"] = json!("inbox");
    doc["nodes"][0]["data"]["config"]["exports"] = json!({"contract":{"entries":{"main":{"input":{"kind":"text"}}}},"entries":{"main":{"node":"wait","inputField":"input"}},"interactive":true});
    let doc: Composition = serde_json::from_value(doc).unwrap();
    let dir = temp.path().join("workspace/.zedflow/flows");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("inbox.rs");
    std::fs::write(
        &path,
        zf_flows::flow_format::render(
            &doc,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let mut start = request(true);
    start.input.insert("input".into(), json!("start"));
    start.definition = StartDefinition::Composition(zf_execution::preparation::RuntimeSelection {
        flow: zf_storage::workspaces::path_id(&path),
        entry: "main".into(),
        bridges: vec![],
        flow_hashes: Default::default(),
        bridge_hashes: Default::default(),
        contexts: Default::default(),
    });
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    idle(&service, &id).await;
    let run = service.read(&actor, &id).await.unwrap();
    assert_eq!(run["wait"]["nodePath"], "root/wait");
    service
        .queue(
            &actor,
            &id,
            MessageRequest {
                id: "qualified".into(),
                kind: "followup".into(),
                text: "continue".into(),
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
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn one_storage_has_one_execution_owner_until_shutdown() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let options = || ExecutionOptions {
        data: temp.path().join("data"),
        workspace: temp.path().join("workspace"),
        flow_home: temp.path().join("home"),
        context_home: Some(temp.path().join("home")),
        skill_dirs: vec![],
        authorizer: Arc::new(Policy::default()),
    };
    let duplicate = ExecutionService::open(options())
        .await
        .err()
        .expect("second owner rejected");
    assert!(matches!(
        duplicate.downcast_ref::<ExecutionError>(),
        Some(ExecutionError::Busy(_))
    ));
    service.shutdown().await.unwrap();
    let replacement = ExecutionService::open(options()).await.unwrap();
    replacement.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_effects_and_keeps_a_resumable_frontier() {
    let temp = tempfile::tempdir().unwrap();
    let service = open(temp.path(), Arc::new(Policy::default())).await;
    let actor = actor(&service);
    let mut doc = serde_json::to_value(flow(false)).unwrap();
    doc["nodes"][1]["data"]["kind"] = json!("tool");
    doc["nodes"][1]["data"]["config"] = json!({"tool":"exec","arguments":{"command":"printf '%s' $$ > root-pid; sleep 20; printf done > unexpected-effect"}});
    let mut start = request(false);
    start.definition = StartDefinition::Inline(serde_json::from_value(doc).unwrap());
    let id = service.start(&actor, start).await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pid = temp.path().join("workspace/root-pid");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !pid.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(service.try_begin_maintenance().is_err());
    tokio::time::timeout(Duration::from_secs(10), service.shutdown())
        .await
        .expect("shutdown drains active run")
        .unwrap();
    assert!(
        !Path::new("/proc")
            .join(std::fs::read_to_string(pid).unwrap())
            .exists()
    );
    let replacement = open(temp.path(), Arc::new(Policy::default())).await;
    let run = replacement.read(&actor, &id).await.unwrap();
    assert!(
        matches!(run["status"].as_str(), Some("stopped" | "interrupted")),
        "{run:#}"
    );
    assert!(run["checkpoint"].is_string());
    assert!(!temp.path().join("workspace/unexpected-effect").exists());
    replacement.shutdown().await.unwrap();
}
