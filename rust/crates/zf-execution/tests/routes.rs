use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{collections::BTreeMap, sync::Arc};
use tempfile::TempDir;
use zf_compiler::{
    graph_compiler::GraphValidator,
    prepared_model::{FrozenFlow, PreparedRuntime},
    resolve,
};
use zf_core::identity::Scope;
use zf_execution::route_runtime::RouteRuntime;
use zf_flows::{
    composition::{CompositionCatalog, InvocationKind, ResolveRequest},
    flow_contract,
    schema::Composition,
};
use zf_runtime::{
    materialize::RuntimePrimitives,
    runtime::{BranchInvocation, DynamicCapabilities, RouteOutcome, RunServices},
    stored_checkpointer::StoredCheckpointer,
    workspace_context::ContextSnapshot,
};
use zf_storage::{content_store::ContentStore, contracts::CheckpointStore, data::DataRegistry};

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
fn prepared(root: Composition, worker: Composition, bridge: Value) -> PreparedRuntime {
    let catalog:CompositionCatalog=serde_json::from_value(json!({"flows":{"root-flow":flow_contract::read(&root).unwrap().unwrap().contract,"worker-flow":flow_contract::read(&worker).unwrap().unwrap().contract},"bridges":{"bridge":bridge}})).unwrap();
    let graph = resolve::resolve(
        &catalog,
        &ResolveRequest {
            flow: "root-flow".into(),
            entry: "main".into(),
            bridges: vec!["bridge".into()],
        },
    )
    .unwrap();
    let flows = BTreeMap::from([
        ("root".into(), freeze(root)),
        ("bridge/worker".into(), freeze(worker)),
    ]);
    PreparedRuntime {
        graph,
        flows,
        definitions: Default::default(),
    }
}
fn freeze(composition: Composition) -> FrozenFlow {
    let source =
        zf_flows::flow_format::render(&composition, &GraphValidator::new(&RuntimePrimitives))
            .unwrap();
    FrozenFlow {
        key: composition.id.clone(),
        hash: format!("{:x}", Sha256::digest(source.as_bytes())),
        source,
        exports: flow_contract::validate(&composition).unwrap().unwrap(),
        composition,
    }
}
struct Fixture {
    _root: TempDir,
    services: Arc<RunServices>,
    store: ContentStore,
    cp: Arc<StoredCheckpointer>,
    runtime: Arc<RouteRuntime>,
    prepared: PreparedRuntime,
}
impl Fixture {
    async fn new(mode: &str, wait: bool) -> Self {
        let (a, b, bridge) = docs(mode, wait);
        Self::with_prepared(prepared(a, b, bridge)).await
    }
    async fn with_prepared(prepared: PreparedRuntime) -> Self {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().join("workspace");
        let data = root.path().join("data");
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(data.join("store.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let store = ContentStore::new(pool.clone()).await.unwrap();
        let services = RunServices::new(
            "route-run".into(),
            cwd.clone(),
            data,
            ContextSnapshot {
                cwd,
                ..Default::default()
            },
            json!({}),
            vec![],
        )
        .unwrap();
        services.set_content_store(store.clone());
        services
            .set_data_registry(
                DataRegistry::new(pool, store.clone(), &services.id)
                    .await
                    .unwrap(),
            )
            .unwrap();
        let cp = Arc::new(StoredCheckpointer::new(
            CheckpointStore::new(store.clone()).await.unwrap(),
        ));
        let runtime = RouteRuntime::new(prepared.clone(), &services, cp.clone(), None).unwrap();
        services.set_dynamic_capabilities(runtime.clone());
        runtime
            .initialize(&State::from([("notes".into(), json!("shared"))]))
            .await
            .unwrap();
        Self {
            _root: root,
            services,
            store,
            cp,
            runtime,
            prepared,
        }
    }
    fn invocation(&self, id: &str) -> BranchInvocation {
        BranchInvocation {
            path: "root/route".into(),
            branch: "delegate".into(),
            invocation: InvocationKind::Node,
            route_id: Some("bridge/call".into()),
            call_id: id.into(),
            input: json!("child input"),
            caller_state: State::from([
                ("notes".into(), json!("shared")),
                ("secret".into(), json!("not exposed")),
            ]),
        }
    }
    fn effects(&self) -> String {
        std::fs::read_to_string(self.services.cwd.join("route-effects.txt")).unwrap()
    }
}

#[tokio::test]
async fn neutral_v4_plug_separates_its_owner_from_authorized_requesters() {
    let (mut root, worker, bridge) = docs("callAwait", false);
    root.format_version = 4;
    let point = &mut root.nodes[0].data.config["exports"]["contract"]["branches"]["delegate"];
    point["invocations"] = json!([]);
    point["requesters"] = json!(["authorized"]);
    for id in ["authorized", "other"] {
        root.nodes
            .push(serde_json::from_value(node(id, "route", json!({"branch":"delegate"}))).unwrap());
    }
    root.edges.retain(|edge| edge.source != "parent-effect");
    for (from, to) in [
        ("parent-effect", "authorized"),
        ("authorized", "other"),
        ("other", "e"),
    ] {
        root.edges
            .push(serde_json::from_value(edge(from, to)).unwrap());
    }
    let source =
        zf_flows::flow_format::render(&root, &GraphValidator::new(&RuntimePrimitives)).unwrap();
    let parsed =
        zf_flows::flow_format::parse(&source, &GraphValidator::new(&RuntimePrimitives)).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::to_value(&root).unwrap()
    );
    let mut historical = root.clone();
    historical.format_version = 3;
    assert!(flow_contract::validate(&historical).is_err());
    let f = Fixture::with_prepared(prepared(root, worker, bridge)).await;
    assert!(
        f.runtime
            .invoke_branch(f.invocation("owner-not-authorized"))
            .await
            .is_err()
    );
    let mut forbidden = f.invocation("other-not-authorized");
    forbidden.path = "root/other".into();
    assert!(f.runtime.invoke_branch(forbidden).await.is_err());
    assert!(!f.services.cwd.join("route-effects.txt").exists());
    let mut allowed = f.invocation("authorized-visit");
    allowed.path = "root/authorized".into();
    assert!(matches!(
        f.runtime.invoke_branch(allowed).await.unwrap(),
        RouteOutcome::Completed { .. }
    ));
    assert_eq!(f.effects(), "x");
}

#[tokio::test]
async fn native_call_await_is_isolated_and_duplicate_visits_reuse_results_after_restart() {
    let f = Fixture::new("callAwait", false).await;
    let first = f
        .runtime
        .invoke_branch(f.invocation("visit-1"))
        .await
        .unwrap();
    let RouteOutcome::Completed {
        thread_id, result, ..
    } = first
    else {
        panic!("Expected completed child")
    };
    assert_eq!(result, "shared");
    assert_eq!(f.effects(), "x");
    let saved = f.cp.load(&thread_id).await.unwrap().unwrap();
    assert!(saved.pending_nodes.is_empty());
    assert_eq!(saved.state["input"], "child input");
    assert_eq!(saved.state["notes"], "shared");
    assert!(!saved.state.contains_key("secret"));
    let registry = f.services.data_registry().unwrap();
    let source = registry
        .snapshot(&Scope::Flow("root".into()), "notes")
        .await
        .unwrap();
    let alias = registry
        .snapshot(&Scope::Flow("bridge/worker".into()), "notes")
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&source.value, &alias.value));
    assert_eq!(source.entity_id, alias.entity_id);
    let restarted = RouteRuntime::new(f.prepared.clone(), &f.services, f.cp.clone(), None).unwrap();
    f.services.set_dynamic_capabilities(restarted.clone());
    restarted.initialize(&State::new()).await.unwrap();
    let again = restarted
        .invoke_branch(f.invocation("visit-1"))
        .await
        .unwrap();
    assert!(matches!(again, RouteOutcome::Completed { .. }));
    assert_eq!(f.effects(), "x");
    restarted
        .invoke_branch(f.invocation("visit-2"))
        .await
        .unwrap();
    assert_eq!(f.effects(), "xx");
    assert_eq!(
        registry
            .snapshot(&Scope::Flow("bridge/worker".into()), "answer")
            .await
            .unwrap()
            .value
            .as_ref(),
        &json!("shared")
    );
}

#[tokio::test]
async fn waiting_child_resumes_its_checkpoint_without_repeating_the_prior_effect() {
    let f = Fixture::new("callAwait", true).await;
    let first = f
        .runtime
        .invoke_branch(f.invocation("visit-wait"))
        .await
        .unwrap();
    let RouteOutcome::Waiting { thread_id, .. } = first else {
        panic!("Expected child wait")
    };
    assert_eq!(f.effects(), "x");
    let saved = f.cp.load(&thread_id).await.unwrap().unwrap();
    assert_eq!(saved.pending_nodes, vec!["gate"]);
    let registry = f.services.data_registry().unwrap();
    let before = registry
        .snapshot(&Scope::Flow("root".into()), "notes")
        .await
        .unwrap();
    registry
        .publish(
            &Scope::Flow("root".into()),
            "notes",
            &before.revision,
            &json!("changed while waiting"),
        )
        .await
        .unwrap();
    let restarted = RouteRuntime::new(f.prepared.clone(), &f.services, f.cp.clone(), None).unwrap();
    f.services.set_dynamic_capabilities(restarted.clone());
    restarted.initialize(&State::new()).await.unwrap();
    let mut invocation = f.invocation("visit-wait");
    invocation.caller_state.insert(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"reply-1","value":"new child input"}),
    );
    let outcome = restarted.invoke_branch(invocation).await.unwrap();
    assert!(matches!(outcome, RouteOutcome::Completed { ref result, .. } if result == "shared"));
    assert_eq!(f.effects(), "x");
    assert_eq!(
        f.cp.load(&thread_id).await.unwrap().unwrap().state["input"],
        "new child input"
    );
}

#[tokio::test]
async fn launch_returns_a_tracked_visit_and_drain_waits_for_native_completion() {
    let f = Fixture::new("launch", false).await;
    let launched = f
        .runtime
        .invoke_branch(f.invocation("launch-1"))
        .await
        .unwrap();
    let RouteOutcome::Launched { visit_id, .. } = launched else {
        panic!("Expected launch")
    };
    assert!(
        f.runtime
            .await_visit("bridge/worker/out", &visit_id, &State::new())
            .await
            .is_err()
    );
    let awaited = f
        .runtime
        .await_visit("root/await", &visit_id, &State::new())
        .await
        .unwrap();
    assert!(matches!(awaited, RouteOutcome::Completed { .. }));
    f.runtime
        .invoke_branch(f.invocation("launch-1"))
        .await
        .unwrap();
    let outcomes = f.runtime.drain().await.unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(outcomes[0], RouteOutcome::Completed { .. }));
    assert_eq!(f.effects(), "x");
    assert_eq!(
        f.runtime.result_snapshot().await.unwrap()[0]["status"],
        "completed"
    );
}

#[tokio::test]
async fn handoff_uses_native_goto_and_does_not_execute_callers_successors() {
    let f = Fixture::new("handoff", false).await;
    let root = flow_contract::at_entry(&f.prepared.flows["root"].composition, "main").unwrap();
    let graph = zf_runtime::materialize::build_with_services(
        &root,
        f.services.clone(),
        None,
        Some(f.cp.clone()),
    )
    .unwrap();
    graph
        .invoke(
            State::from([
                ("input".into(), json!("child input")),
                ("notes".into(), json!("shared")),
            ]),
            ExecutionConfig::new(&f.services.id),
        )
        .await
        .unwrap();
    assert_eq!(f.effects(), "x");
    assert!(!f.services.cwd.join("parent-effects.txt").exists());
}

#[tokio::test]
async fn typed_route_rejects_incompatible_input_and_unknown_caller_before_effect() {
    let f = Fixture::new("callAwait", false).await;
    let mut wrong = f.invocation("bad");
    wrong.input = json!(42);
    assert!(f.runtime.invoke_branch(wrong).await.is_err());
    let mut wrong = f.invocation("bad-owner");
    wrong.path = "root/other-node".into();
    assert!(f.runtime.invoke_branch(wrong).await.is_err());
    assert!(!f.services.cwd.join("route-effects.txt").exists());
    assert!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .all(|record| record.kind != "route-visits")
    );
}

#[tokio::test]
async fn inactive_condition_skips_without_a_child_visit_or_effect() {
    let (a, b, mut bridge) = docs("callAwait", false);
    bridge["connections"]["call"]["invocation"] = json!("condition");
    bridge["connections"]["call"]["condition"] =
        json!({"kind":"compare","field":"enabled","operator":"eq","value":true});
    let f = Fixture::with_prepared(prepared(a, b, bridge)).await;
    let mut invocation = f.invocation("skip");
    invocation.invocation = InvocationKind::Condition;
    let mut missing = invocation.clone();
    missing.call_id = "missing-selection".into();
    missing.route_id = Some("bridge/missing".into());
    let error = f.runtime.invoke_branch(missing).await.unwrap_err();
    assert!(error.to_string().contains("does not belong"), "{error:#}");
    invocation
        .caller_state
        .insert("enabled".into(), json!(false));
    invocation
        .caller_state
        .insert("notes".into(), json!("updated before skipped route"));
    assert!(matches!(
        f.runtime.invoke_branch(invocation).await.unwrap(),
        RouteOutcome::Skipped { .. }
    ));
    assert!(!f.services.cwd.join("route-effects.txt").exists());
    assert_eq!(
        f.services
            .data_registry()
            .unwrap()
            .snapshot(&Scope::Flow("bridge/worker".into()), "notes")
            .await
            .unwrap()
            .value
            .as_ref(),
        &json!("updated before skipped route")
    );
    assert!(
        f.store
            .records(&f.services.id)
            .await
            .unwrap()
            .iter()
            .all(|r| r.kind != "route-visits")
    );
}

#[tokio::test]
async fn model_tool_route_waits_without_deadlock_and_resumes_one_durable_call() {
    use adk_graph::{Node, NodeContext};
    use zf_context::{
        context::{
            ContextBlock, ContextCapability, ContextExpr, ContextStrategy, FragmentFormat,
            FragmentRole,
        },
        context_source,
    };
    use zf_core::types::DataType;
    use zf_storage::context_store;
    let capability = ContextCapability::new(
        "delegate",
        DataType::Record {
            fields: BTreeMap::from([("input".into(), DataType::Text)]),
        },
        DataType::Text,
    );
    let strategy = ContextStrategy::new("delegate-context", "Delegate")
        .capability(capability.clone())
        .with_program(vec![ContextBlock::emit(
            "task",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::literal(DataType::Text, json!("Call the worker")),
        )]);
    let source = context_source::generate(&strategy).unwrap();
    let config = json!({"__zedflowVersion":3,"provider":"fixture","capabilityGrants":[capability],"contextProgram":{"strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),"types":{},"bindings":{}},"fixtureSteps":[{"tool":"delegate","args":{"input":"child input"}}]});
    let (root, child, mut bridge) = docs("callAwait", true);
    let mut root = serde_json::to_value(root).unwrap();
    root["nodes"][0]["data"]["config"]["exports"]["contract"]["branches"]["delegate"]["invocations"] =
        json!(["tool"]);
    root["nodes"][1]["data"]["kind"] = json!("agent");
    root["nodes"][1]["data"]["config"] = config.clone();
    root["nodes"][1]["data"]["config"]
        .as_object_mut()
        .unwrap()
        .remove("__zedflowVersion");
    bridge["connections"]["call"]["invocation"] = json!("tool");
    bridge["connections"]["call"]["toolName"] = json!("delegate");
    let f = Fixture::with_prepared(prepared(
        serde_json::from_value(root).unwrap(),
        child,
        bridge,
    ))
    .await;
    let model =
        zf_runtime::models::node_with_services("route", &config, "root/route", f.services.clone())
            .unwrap();
    let output = model
        .execute(&NodeContext::new(
            State::from([("notes".into(), json!("shared"))]),
            ExecutionConfig::new(&f.services.id),
            0,
        ))
        .await
        .unwrap();
    let state = output.updates;
    let tool_config = json!({"__zedflowVersion":3,"tool":"execute_next_call"});
    let dispatch = |state| {
        zf_runtime::operations::execute_with_services(
            "tool",
            &tool_config,
            NodeContext::new(state, ExecutionConfig::new(&f.services.id), 1),
            "root/dispatch",
            f.services.clone(),
        )
    };
    let waiting = tokio::time::timeout(std::time::Duration::from_secs(5), dispatch(state.clone()))
        .await
        .expect("child tools must not deadlock on caller journal")
        .unwrap();
    assert!(
        waiting.interrupt.is_some(),
        "Expected child input wait: {:?}",
        waiting.updates
    );
    assert!(
        waiting.updates["toolResults"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.effects(), "x");
    let mut resume = state;
    resume.insert(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"tool-reply","value":"continue"}),
    );
    let resumed = tokio::time::timeout(std::time::Duration::from_secs(5), dispatch(resume))
        .await
        .unwrap()
        .unwrap();
    assert!(resumed.interrupt.is_none());
    assert_eq!(
        resumed.updates["toolResults"][0]["result"]["__zedflowRoute"]["status"],
        "completed"
    );
    assert_eq!(
        resumed.updates["toolResults"][0]["result"]["result"],
        "shared"
    );
    assert_eq!(f.effects(), "x");
    let receipts = f
        .store
        .records(&f.services.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == "receipts")
        .collect::<Vec<_>>();
    assert_eq!(
        receipts.len(),
        2,
        "one model-routed call and one child shell effect"
    );
    for receipt in receipts {
        assert_eq!(
            f.store.resolve(&receipt.value_ref).await.unwrap()["status"],
            "completed"
        );
    }
}

#[tokio::test]
async fn concurrent_child_outputs_use_the_heads_captured_before_execution() {
    let f = Fixture::new("callAwait", true).await;
    for id in ["writer-a", "writer-b"] {
        assert!(matches!(
            f.runtime.invoke_branch(f.invocation(id)).await.unwrap(),
            RouteOutcome::Waiting { .. }
        ));
    }
    assert_eq!(f.effects(), "xx");
    let mut first = f.invocation("writer-a");
    first.caller_state.insert(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"a","value":"continue"}),
    );
    f.runtime.invoke_branch(first).await.unwrap();
    let registry = f.services.data_registry().unwrap();
    let published = registry
        .snapshot(&Scope::Flow("bridge/worker".into()), "answer")
        .await
        .unwrap();
    let mut stale = f.invocation("writer-b");
    stale.caller_state.insert(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"b","value":"continue"}),
    );
    assert!(f.runtime.invoke_branch(stale.clone()).await.is_err());
    assert!(f.runtime.invoke_branch(stale).await.is_err());
    let after = registry
        .snapshot(&Scope::Flow("bridge/worker".into()), "answer")
        .await
        .unwrap();
    assert_eq!(published.revision, after.revision);
    assert_eq!(
        f.effects(),
        "xx",
        "completed checkpoints must not rerun either effect on publication conflict"
    );
}

#[tokio::test]
async fn unchanged_channel_preserves_alias_edits_and_changed_stale_channel_conflicts() {
    let f = Fixture::new("callAwait", false).await;
    let registry = f.services.data_registry().unwrap();
    let old = registry
        .snapshot(&Scope::Flow("root".into()), "notes")
        .await
        .unwrap();
    let edited = registry
        .publish(
            &Scope::Flow("root".into()),
            "notes",
            &old.revision,
            &json!("edited via alias"),
        )
        .await
        .unwrap();
    f.runtime
        .capture(
            "root/route",
            "unchanged",
            &State::from([("notes".into(), json!("shared"))]),
        )
        .await
        .unwrap();
    assert_eq!(
        registry
            .snapshot(&Scope::Flow("root".into()), "notes")
            .await
            .unwrap()
            .revision,
        edited.revision
    );
    assert!(
        f.runtime
            .capture(
                "root/route",
                "stale-change",
                &State::from([("notes".into(), json!("locally changed"))])
            )
            .await
            .is_err()
    );
    let restarted = RouteRuntime::new(f.prepared.clone(), &f.services, f.cp.clone(), None).unwrap();
    restarted.initialize(&State::new()).await.unwrap();
    restarted
        .capture(
            "root/route",
            "still-unchanged",
            &State::from([("notes".into(), json!("shared"))]),
        )
        .await
        .unwrap();
    assert_eq!(
        registry
            .snapshot(&Scope::Flow("root".into()), "notes")
            .await
            .unwrap()
            .revision,
        edited.revision
    );
}

#[tokio::test]
async fn nested_launches_retain_a_durable_runtime_depth_budget() {
    let (root, worker, mut bridge) = docs("launch", false);
    let mut root = serde_json::to_value(root).unwrap();
    root["settings"]["recursionLimit"] = json!(4);
    let mut worker = serde_json::to_value(worker).unwrap();
    worker["nodes"][0]["data"]["config"]["exports"]["contract"]["branches"] =
        json!({"back":{"contract":{"input":{"kind":"text"}},"invocations":["node"]}});
    worker["nodes"][0]["data"]["config"]["exports"]["branches"] = json!({"back":"back"});
    worker["nodes"][0]["data"]["config"]["exports"]["data"]["answer"] = json!("answer");
    worker["nodes"][0]["data"]["config"]["exports"]["entries"]["main"]["outputField"] =
        json!("answer");
    worker["channels"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"answer","reducer":"overwrite"}));
    for node in worker["nodes"].as_array_mut().unwrap() {
        if node["id"] == "out" {
            node["data"]["config"]["field"] = json!("answer");
        }
    }

    worker["nodes"].as_array_mut().unwrap().push(node(
        "back",
        "route",
        json!({"branch":"back","mode":"launch"}),
    ));
    worker["edges"] = json!([
        edge("s", "effect"),
        edge("effect", "back"),
        edge("back", "out"),
        edge("out", "e")
    ]);
    bridge["connections"]["back"] = json!({"from":{"instance":"worker","port":"back"},"to":{"instance":"root","port":"main"},"invocation":"node","mode":"launch"});
    let f = Fixture::with_prepared(prepared(
        serde_json::from_value(root).unwrap(),
        serde_json::from_value(worker).unwrap(),
        bridge,
    ))
    .await;
    f.runtime
        .invoke_branch(f.invocation("depth-root"))
        .await
        .unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(5), f.runtime.drain())
        .await
        .expect("launch cycles must stop")
        .unwrap_err();
    assert!(error.to_string().contains("Route depth"), "{error:#}");
    let mut depths = Vec::new();
    for record in f
        .store
        .records(&f.services.id)
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.kind == "route-visits")
    {
        let visit = f.store.resolve(&record.value_ref).await.unwrap();
        let depth = visit["depth"].as_u64().unwrap();
        assert!(depth <= 4);
        assert_eq!(visit["parentVisitId"].is_string(), depth > 1);
        depths.push(depth);
    }
    depths.sort();
    assert_eq!(depths, vec![1, 2, 3, 4]);
}

#[tokio::test]
async fn routed_child_rebuilds_only_its_sequential_boundary_without_exposing_a_wait() {
    use zf_runtime::revisions::{Compatibility, RevisionDefinition, RevisionRuntime};
    let (root, mut worker, bridge) = docs("callAwait", false);
    worker
        .nodes
        .iter_mut()
        .find(|node| node.id == "effect")
        .unwrap()
        .data
        .config = json!({"tool":"exec","arguments":{"command":"printf x >> route-effects.txt; while [ ! -f release-child ]; do sleep 0.01; done","timeoutMs":10000}});
    let f = Fixture::with_prepared(prepared(root, worker, bridge)).await;
    let definitions = f
        .prepared
        .flows
        .iter()
        .map(|(instance, flow)| {
            (
                instance.clone(),
                RevisionDefinition {
                    package: None,
                    context_selections: Default::default(),
                    key: flow.key.clone(),
                    hash: flow.hash.clone(),
                    source: flow.source.clone(),
                    composition: flow.composition.clone(),
                },
            )
        })
        .collect();
    let revisions = RevisionRuntime::new(f.store.clone(), &f.services.id, definitions)
        .await
        .unwrap();
    f.services.set_revisions(revisions.clone()).unwrap();
    let mut child = serde_json::to_value(&f.prepared.flows["bridge/worker"].composition).unwrap();
    child["nodes"].as_array_mut().unwrap().push(node(
        "extra",
        "set",
        json!({"field":"output","value":"intermediate"}),
    ));
    child["edges"] = json!([
        edge("s", "effect"),
        edge("effect", "out"),
        edge("out", "extra"),
        edge("extra", "e")
    ]);
    let flow = freeze(serde_json::from_value(child).unwrap());
    let invocation = f.invocation("adopt-child");
    let running = tokio::spawn({
        let runtime = f.runtime.clone();
        async move { runtime.invoke_branch(invocation).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !f.services.cwd.join("route-effects.txt").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        revisions
            .publish(
                "bridge/worker",
                RevisionDefinition {
                    package: None,
                    context_selections: Default::default(),
                    key: flow.key,
                    hash: flow.hash.clone(),
                    source: flow.source,
                    composition: flow.composition
                }
            )
            .await
            .unwrap(),
        Compatibility::SequentialBoundary { .. }
    ));
    std::fs::write(f.services.cwd.join("release-child"), b"continue").unwrap();
    let completed = running.await.unwrap().unwrap();
    assert!(matches!(completed,RouteOutcome::Completed{ref result,..}if result=="intermediate"));
    assert_eq!(f.effects(), "x");
    assert!(
        Arc::ptr_eq(&revisions, &f.services.revisions().unwrap()),
        "rebasing a child must not replace the controller used by other executors"
    );
    let steps = f.store.records(&f.services.id).await.unwrap();
    let mut found = false;
    for step in steps.into_iter().filter(|r| r.kind == "revision-steps") {
        let pin = f.store.resolve(&step.value_ref).await.unwrap();
        if pin["scope"] == "bridge/worker" && pin["step"].as_u64().unwrap() > 0 {
            assert_eq!(pin["hash"], flow.hash);
            found = true;
        }
    }
    assert!(found);
}

fn alternate_entry(mode: &str, tool: bool) -> (PreparedRuntime, PreparedRuntime) {
    let (root, worker, mut bridge) = docs(mode, true);
    let mut root = serde_json::to_value(root).unwrap();
    let mut worker = serde_json::to_value(worker).unwrap();
    worker["nodes"][0]["data"]["config"]["exports"]["contract"]["entries"]["fast"] =
        json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    worker["nodes"][0]["data"]["config"]["exports"]["entries"]["fast"] =
        json!({"node":"out","inputField":"input","outputField":"output"});
    if tool {
        root["nodes"][0]["data"]["config"]["exports"]["contract"]["branches"]["delegate"]["invocations"] =
            json!(["tool"]);
        root["nodes"][1]["data"]["kind"] = json!("agent");
        root["nodes"][1]["data"]["config"] = json!({"provider":"fixture"});
        bridge["connections"]["call"]["invocation"] = json!("tool");
        bridge["connections"]["call"]["toolName"] = json!("delegate");
    }
    let root: Composition = serde_json::from_value(root).unwrap();
    let worker: Composition = serde_json::from_value(worker).unwrap();
    let before = prepared(root.clone(), worker.clone(), bridge.clone());
    bridge["connections"]["call"]["to"]["port"] = json!("fast");
    (before, prepared(root, worker, bridge))
}

async fn publish_plan(f: &Fixture, plan: PreparedRuntime) {
    use zf_runtime::revisions::{RuntimeGraphPublication, publish_mixed_unique};
    publish_mixed_unique(
        &f.store,
        &uuid::Uuid::new_v4().to_string(),
        &[],
        &[RuntimeGraphPublication {
            run_id: f.services.id.clone(),
            baseline: f.prepared.clone(),
            prepared: plan,
        }],
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn published_bridge_changes_future_visits_but_waiting_visit_keeps_original_entry_after_restart()
 {
    let (before, after) = alternate_entry("callAwait", false);
    let f = Fixture::with_prepared(before).await;
    let first = f.runtime.invoke_branch(f.invocation("old")).await.unwrap();
    let RouteOutcome::Waiting { visit_id, .. } = first else {
        panic!("expected original input wait")
    };
    let captured = f
        .services
        .read_record("route-visits", &visit_id)
        .await
        .unwrap()
        .unwrap();
    publish_plan(&f, after).await;
    let restarted = RouteRuntime::new(f.prepared.clone(), &f.services, f.cp.clone(), None).unwrap();
    f.services.set_dynamic_capabilities(restarted.clone());
    restarted.initialize(&State::new()).await.unwrap();
    let mut old = f.invocation("old");
    old.caller_state.insert(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"old-answer","value":"continue"}),
    );
    assert!(matches!(
        restarted.invoke_branch(old).await.unwrap(),
        RouteOutcome::Completed { .. }
    ));
    assert_eq!(
        f.effects(),
        "x",
        "old checkpoint resumes without replaying the shell"
    );
    let next = restarted.invoke_branch(f.invocation("new")).await.unwrap();
    let RouteOutcome::Completed {
        visit_id: next_id, ..
    } = next
    else {
        panic!("new route must use the fast entry without waiting")
    };
    let next = f
        .services
        .read_record("route-visits", &next_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(captured["entry"], "main");
    assert_eq!(next["entry"], "fast");
    assert_ne!(captured["graphRef"], next["graphRef"]);
    assert_eq!(
        f.services
            .read_record("route-visits", &visit_id)
            .await
            .unwrap()
            .unwrap(),
        captured
    );
    assert_eq!(f.effects(), "x");
}

#[tokio::test]
async fn captured_model_tool_uses_old_graph_when_save_happens_before_dispatch() {
    let (before, after) = alternate_entry("callAwait", true);
    let f = Fixture::with_prepared(before).await;
    let old_invocation = uuid::Uuid::new_v4().to_string();
    let state = State::from([("notes".into(), json!("shared"))]);
    f.runtime
        .capture("root/route", &old_invocation, &state)
        .await
        .unwrap();
    publish_plan(&f, after).await;
    let marker = f
        .runtime
        .invoke(
            "root/route",
            &format!("{old_invocation}:0"),
            "delegate",
            json!({"input":"task"}),
        )
        .await
        .unwrap();
    assert_eq!(marker["__zedflowRoute"]["status"], "waiting");
    let visit_id = marker["__zedflowRoute"]["visitId"].as_str().unwrap();
    f.runtime
        .await_visit(
            "root/route",
            visit_id,
            &State::from([(
                "answer:bridge/worker/gate".into(),
                json!({"__zedflowAnswerId":"tool-answer","value":"continue"}),
            )]),
        )
        .await
        .unwrap();
    let next_invocation = uuid::Uuid::new_v4().to_string();
    f.runtime
        .capture("root/route", &next_invocation, &state)
        .await
        .unwrap();
    let next = f
        .runtime
        .invoke(
            "root/route",
            &format!("{next_invocation}:0"),
            "delegate",
            json!({"input":"task"}),
        )
        .await
        .unwrap();
    assert_eq!(next["__zedflowRoute"]["status"], "completed");
    assert_eq!(f.effects(), "x");
    let old = f
        .services
        .read_record("route-inputs", &old_invocation)
        .await
        .unwrap()
        .unwrap();
    let new = f
        .services
        .read_record("route-inputs", &next_invocation)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(old["graphRef"], new["graphRef"]);
}

#[tokio::test]
async fn mixed_graph_publication_is_atomic_replayable_and_rejects_runtime_structure() {
    use zf_runtime::revisions::{
        Compatibility, RevisionDefinition, RevisionPublication, RevisionRuntime,
        RuntimeGraphPublication, publish_mixed_unique, runtime_graph_compatibility,
    };
    let (before, after) = alternate_entry("callAwait", false);
    let f = Fixture::with_prepared(before).await;
    let definitions = f
        .prepared
        .flows
        .iter()
        .map(|(instance, flow)| {
            (
                instance.clone(),
                RevisionDefinition {
                    package: None,
                    context_selections: Default::default(),
                    key: flow.key.clone(),
                    hash: flow.hash.clone(),
                    source: flow.source.clone(),
                    composition: flow.composition.clone(),
                },
            )
        })
        .collect();
    let controller = RevisionRuntime::new(f.store.clone(), &f.services.id, definitions)
        .await
        .unwrap();
    f.services.set_revisions(controller).unwrap();
    let old = &f.prepared.flows["bridge/worker"];
    let mut updated = old.composition.clone();
    updated
        .nodes
        .iter_mut()
        .find(|n| n.id == "out")
        .unwrap()
        .data
        .config["value"] = json!("updated source");
    let updated = freeze(updated);
    let publication = RevisionPublication {
        run_id: f.services.id.clone(),
        instance: "bridge/worker".into(),
        baseline: old.composition.clone(),
        definition: RevisionDefinition {
            package: None,
            context_selections: Default::default(),
            key: updated.key,
            hash: updated.hash.clone(),
            source: updated.source,
            composition: updated.composition,
        },
    };
    let graph = RuntimeGraphPublication {
        run_id: f.services.id.clone(),
        baseline: f.prepared.clone(),
        prepared: after,
    };
    let records_before = f.store.records(&f.services.id).await.unwrap();
    let batch = uuid::Uuid::new_v4().to_string();
    sqlx::query("CREATE TRIGGER reject_graph_update BEFORE UPDATE ON zf_records WHEN NEW.kind='runtime-graph-heads' BEGIN SELECT RAISE(ABORT,'fixture failure'); END").execute(f.store.pool()).await.unwrap();
    assert!(
        publish_mixed_unique(
            &f.store,
            &batch,
            std::slice::from_ref(&publication),
            std::slice::from_ref(&graph)
        )
        .await
        .is_err()
    );
    assert_eq!(
        json!(f.store.records(&f.services.id).await.unwrap()),
        json!(records_before)
    );
    sqlx::query("DROP TRIGGER reject_graph_update")
        .execute(f.store.pool())
        .await
        .unwrap();
    publish_mixed_unique(
        &f.store,
        &batch,
        std::slice::from_ref(&publication),
        std::slice::from_ref(&graph),
    )
    .await
    .unwrap();
    let descriptor = f
        .runtime
        .route_contract("root/route", "delegate", Some("bridge/call"))
        .await
        .unwrap();
    assert_eq!(
        descriptor["targetHash"], updated.hash,
        "producer fingerprint follows the published flow head"
    );
    let newer = uuid::Uuid::new_v4().to_string();
    let reversed = RuntimeGraphPublication {
        run_id: f.services.id.clone(),
        baseline: graph.prepared.clone(),
        prepared: f.prepared.clone(),
    };
    publish_mixed_unique(&f.store, &newer, &[], &[reversed])
        .await
        .unwrap();
    let head = f
        .store
        .record(&f.services.id, "runtime-graph-heads", "current")
        .await
        .unwrap();
    publish_mixed_unique(
        &f.store,
        &batch,
        std::slice::from_ref(&publication),
        std::slice::from_ref(&graph),
    )
    .await
    .unwrap();
    assert_eq!(
        head,
        f.store
            .record(&f.services.id, "runtime-graph-heads", "current")
            .await
            .unwrap()
    );
    assert!(
        publish_mixed_unique(&f.store, &batch, &[], std::slice::from_ref(&graph))
            .await
            .is_err()
    );
    let mut changed = serde_json::to_value(f.prepared.graph.bridges["bridge"].clone()).unwrap();
    changed["connections"]["new-call"] = changed["connections"]["call"].clone();
    let changed = prepared(
        f.prepared.flows["root"].composition.clone(),
        old.composition.clone(),
        changed,
    );
    assert!(matches!(
        runtime_graph_compatibility(&f.prepared, &changed).unwrap(),
        Compatibility::Incompatible { .. }
    ));
}

#[tokio::test]
async fn native_parallel_step_pins_graph_before_any_model_capture() {
    use adk_graph::{Node, NodeOutput, StateGraph, node::FunctionNode};
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::Semaphore;
    use zf_runtime::revisions::{NodeFactory, RevisionDefinition, RevisionRuntime};
    let (before, after) = alternate_entry("callAwait", false);
    let f = Fixture::with_prepared(before).await;
    let definitions = f
        .prepared
        .flows
        .iter()
        .map(|(instance, flow)| {
            (
                instance.clone(),
                RevisionDefinition {
                    package: None,
                    context_selections: Default::default(),
                    key: flow.key.clone(),
                    hash: flow.hash.clone(),
                    source: flow.source.clone(),
                    composition: flow.composition.clone(),
                },
            )
        })
        .collect();
    let revisions = RevisionRuntime::new(f.store.clone(), &f.services.id, definitions)
        .await
        .unwrap();
    f.services.set_revisions(revisions.clone()).unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let once = Arc::new(AtomicBool::new(false));
    let factory: NodeFactory = Arc::new({
        let runtime = f.runtime.clone();
        let entered = entered.clone();
        let release = release.clone();
        move |_, spec| {
            let (runtime, entered, release, once, id) = (
                runtime.clone(),
                entered.clone(),
                release.clone(),
                once.clone(),
                spec.id.clone(),
            );
            Ok(Arc::new(FunctionNode::new(&spec.id, move |ctx| {
                let (runtime, entered, release, once, id) = (
                    runtime.clone(),
                    entered.clone(),
                    release.clone(),
                    once.clone(),
                    id.clone(),
                );
                async move {
                    if !once.swap(true, Ordering::SeqCst) {
                        entered.add_permits(1);
                        release.acquire().await.unwrap().forget();
                    }
                    runtime
                        .capture(
                            "root/route",
                            &format!("{}-{id}", ctx.config.thread_id),
                            &State::from([("notes".into(), json!("shared"))]),
                        )
                        .await
                        .map_err(|e| adk_graph::error::GraphError::NodeExecutionFailed {
                            node: id.clone(),
                            message: e.to_string(),
                        })?;
                    Ok(NodeOutput::new())
                }
            })) as Arc<dyn Node>)
        }
    });
    let mut graph = StateGraph::new(zf_runtime::operations::state_schema(&json!([])).unwrap());
    for id in ["route", "parent-effect"] {
        let spec = f.prepared.flows["root"]
            .composition
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap();
        graph.nodes.insert(
            id.into(),
            revisions.wrap(
                "root",
                id,
                factory(&f.prepared.flows["root"].composition, spec).unwrap(),
                factory.clone(),
            ),
        );
        graph = graph
            .add_edge(adk_graph::START, id)
            .add_edge(id, adk_graph::END);
    }
    let graph = Arc::new(
        graph
            .compile()
            .unwrap()
            .with_checkpointer_arc(f.cp.clone())
            .with_max_concurrency(1),
    );
    let running = tokio::spawn({
        let graph = graph.clone();
        async move {
            graph
                .invoke(State::new(), ExecutionConfig::new("parallel-old"))
                .await
        }
    });
    entered.acquire().await.unwrap().forget();
    publish_plan(&f, after).await;
    release.add_permits(1);
    running.await.unwrap().unwrap();
    let left = f
        .services
        .read_record("route-inputs", "parallel-old-route")
        .await
        .unwrap()
        .unwrap();
    let right = f
        .services
        .read_record("route-inputs", "parallel-old-parent-effect")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(left["graphRef"], right["graphRef"]);
    let old_plan: PreparedRuntime = serde_json::from_value(
        f.store
            .resolve(left["graphRef"].as_str().unwrap())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(old_plan.graph.routes["bridge/call"].to.port, "main");
    graph
        .invoke(State::new(), ExecutionConfig::new("parallel-next"))
        .await
        .unwrap();
    let next = f
        .services
        .read_record("route-inputs", "parallel-next-route")
        .await
        .unwrap()
        .unwrap();
    let next_plan: PreparedRuntime = serde_json::from_value(
        f.store
            .resolve(next["graphRef"].as_str().unwrap())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(next_plan.graph.routes["bridge/call"].to.port, "fast");
    assert_ne!(left["graphRef"], next["graphRef"]);
}

#[tokio::test]
async fn restarted_host_recovers_a_launched_wait_without_repeating_its_effect() {
    let f = Fixture::new("launch", true).await;
    let RouteOutcome::Launched {
        visit_id,
        thread_id,
        ..
    } = f
        .runtime
        .invoke_branch(f.invocation("recover-launch"))
        .await
        .unwrap()
    else {
        panic!("expected launched child");
    };
    let first = f.runtime.drain().await.unwrap();
    assert!(matches!(first.as_slice(), [RouteOutcome::Waiting { .. }]));
    assert_eq!(f.effects(), "x");
    let restarted = RouteRuntime::new(f.prepared.clone(), &f.services, f.cp.clone(), None).unwrap();
    f.services.set_dynamic_capabilities(restarted.clone());
    restarted.initialize(&State::new()).await.unwrap();
    let recovered = restarted.drain().await.unwrap();
    assert!(
        matches!(recovered.as_slice(), [RouteOutcome::Waiting { thread_id: recovered_thread, .. }] if recovered_thread == &thread_id)
    );
    let answered = State::from([(
        "answer:bridge/worker/gate".into(),
        json!({"__zedflowAnswerId":"recovered-answer","value":"continue"}),
    )]);
    let completed = restarted
        .await_visit("root/await", &visit_id, &answered)
        .await
        .unwrap();
    assert!(matches!(completed, RouteOutcome::Completed { result, .. } if result == "shared"));
    assert_eq!(f.effects(), "x");
}

#[tokio::test]
async fn dispatch_and_model_capture_identities_reject_changed_inputs() {
    let f = Fixture::new("callAwait", false).await;
    f.runtime
        .invoke_branch(f.invocation("sealed-dispatch"))
        .await
        .unwrap();
    let mut altered = f.invocation("sealed-dispatch");
    altered.input = json!("changed input");
    let error = f.runtime.invoke_branch(altered).await.unwrap_err();
    assert!(error.to_string().contains("identity reused"), "{error:#}");
    let original = State::from([("notes".into(), json!("shared"))]);
    f.runtime
        .capture("root/route", "sealed-model", &original)
        .await
        .unwrap();
    let changed = State::from([("notes".into(), json!("changed input"))]);
    let error = f
        .runtime
        .capture("root/route", "sealed-model", &changed)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("identity reused"), "{error:#}");
    assert_eq!(f.effects(), "x");
}

mod portable_export {
    use adk_graph::State;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use zf_compiler::{
        graph_compiler::GraphValidator,
        prepared_model::{FrozenFlow, PreparedRuntime},
        resolve,
    };
    use zf_execution::{
        route_runtime::NativeFactory,
        runtime_export::{self, RunOptions},
    };
    use zf_flows::{
        composition::{CompositionCatalog, ResolveRequest},
        flow_contract,
        schema::Composition,
    };
    use zf_runtime::{
        materialize::RuntimePrimitives,
        revisions::{Compatibility, RevisionDefinition, RevisionRuntime},
    };
    use zf_storage::content_store::ContentStore;

    fn doc() -> Composition {
        serde_json::from_value(json!({"formatVersion":3,"id":"portable-root","name":"Portable root","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":true}}}},
        {"id":"gate","position":{"x":0,"y":100},"data":{"kind":"set","label":"Gate","config":{"field":"output","value":"old"}}},
        {"id":"pause","position":{"x":0,"y":150},"data":{"kind":"input","label":"Pause","config":{"field":"output","prompt":"Question","responseType":"text"}}},
        {"id":"publish","position":{"x":0,"y":200},"data":{"kind":"output","label":"Publish","config":{"inputField":"output"}}},
        {"id":"end","position":{"x":0,"y":300},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"start","target":"gate"},{"id":"b","source":"gate","target":"pause"},{"id":"p","source":"pause","target":"publish"},{"id":"c","source":"publish","target":"end"}]})).unwrap()
    }
    fn definition(composition: Composition) -> RevisionDefinition {
        let source =
            zf_flows::flow_format::render(&composition, &GraphValidator::new(&RuntimePrimitives))
                .unwrap();
        RevisionDefinition {
            package: None,
            context_selections: Default::default(),
            key: composition.id.clone(),
            hash: format!("{:x}", Sha256::digest(source.as_bytes())),
            source,
            composition,
        }
    }
    #[tokio::test]
    async fn portable_root_consumes_structural_boundaries_and_resumes_the_checkpoint_definition() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        let data = temp.path().join("data");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        // The production entrypoint discovers workspace context only when no
        // captured context exists. Seed this fixture explicitly to keep host
        // instruction/skill discovery outside the test.
        let run_data = data.join("export-resume");
        std::fs::create_dir_all(&run_data).unwrap();
        std::fs::write(
            run_data.join("context.json"),
            serde_json::to_vec(&zf_runtime::workspace_context::ContextSnapshot {
                cwd: workspace.clone(),
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        let old = definition(doc());
        let exports = flow_contract::read(&old.composition).unwrap().unwrap();
        let catalog = CompositionCatalog {
            flows: BTreeMap::from([(old.key.clone(), exports.contract.clone())]),
            ..Default::default()
        };
        let graph = resolve::resolve(
            &catalog,
            &ResolveRequest {
                flow: old.key.clone(),
                entry: "main".into(),
                bridges: vec![],
            },
        )
        .unwrap();
        let prepared = PreparedRuntime {
            graph,
            flows: BTreeMap::from([(
                "root".into(),
                FrozenFlow {
                    key: old.key.clone(),
                    hash: old.hash.clone(),
                    source: old.source.clone(),
                    composition: old.composition.clone(),
                    exports,
                },
            )]),
            definitions: Default::default(),
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(data.join("sessions.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let content = ContentStore::new(pool).await.unwrap();
        let revisions = RevisionRuntime::new(
            content,
            "export-resume",
            BTreeMap::from([("root".into(), old.clone())]),
        )
        .await
        .unwrap();
        let mut updated = old.composition.clone();
        updated.nodes.push(serde_json::from_value(json!({"id":"new_tail","position":{"x":0,"y":170},"data":{"kind":"set","label":"New tail","config":{"field":"output","value":"adopted {{output}}"}}})).unwrap());
        updated
            .edges
            .iter_mut()
            .find(|edge| edge.source == "pause")
            .unwrap()
            .target = "new_tail".into();
        updated.edges.push(
            serde_json::from_value(json!({"id":"added","source":"new_tail","target":"publish"}))
                .unwrap(),
        );
        assert!(matches!(
            revisions
                .publish("root", definition(updated))
                .await
                .unwrap(),
            Compatibility::SequentialBoundary { .. }
        ));
        let called = Arc::new(AtomicUsize::new(0));
        let factory: NativeFactory = {
            let called = called.clone();
            let doc = old.composition.clone();
            Arc::new(move |_, _, _| {
                called.fetch_add(1, Ordering::SeqCst);
                zf_runtime::materialize::build(&doc)
            })
        };
        let options = |input: Value| RunOptions {
            workspace: workspace.clone(),
            home: Some(workspace.join(".fixture-home")),
            data: data.clone(),
            run_id: "export-resume".into(),
            input: serde_json::from_value::<State>(input).unwrap(),
            models: None,
            capabilities: None,
        };
        let waiting = runtime_export::run(
            prepared.clone(),
            BTreeMap::from([("root".into(), factory.clone())]),
            options(json!({"input":"begin"})),
        )
        .await
        .unwrap();
        assert_eq!(waiting["status"], "waiting", "{waiting}");
        assert!(
            waiting["interrupt"].to_string().contains("root/pause"),
            "{waiting}"
        );
        assert!(
            !waiting["interrupt"]
                .to_string()
                .contains("revision_boundary"),
            "{waiting}"
        );
        assert_eq!(called.load(Ordering::SeqCst), 1);
        let resumed = runtime_export::run(
            prepared,
            BTreeMap::from([("root".into(), factory)]),
            options(json!({"answer:pause":"from adopted source"})),
        )
        .await
        .unwrap();
        assert_eq!(resumed["status"], "completed", "{resumed}");
        assert_eq!(resumed["state"]["response"], "adopted from adopted source");
        assert_eq!(
            called.load(Ordering::SeqCst),
            1,
            "initial native source must not reconstruct an adopted structure"
        );
        let events = std::fs::read_to_string(data.join("export-resume/events.jsonl")).unwrap();
        assert!(events.contains("revision_adopted"));
    }
}
