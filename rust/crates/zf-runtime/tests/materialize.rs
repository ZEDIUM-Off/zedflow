use adk_graph::{ExecutionConfig, State};
use serde_json::{Value, json};
use zf_flows::schema::Composition;
use zf_runtime::materialize;

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn composition(nodes: Vec<Value>, edges: &[(&str, &str)]) -> Value {
    json!({"id":"parameters","name":"Paramètres ADK","nodes":nodes,"edges":edges.iter().enumerate().map(|(i,(source,target))|json!({"id":i.to_string(),"source":source,"target":target})).collect::<Vec<_>>()})
}
fn parse(value: Value) -> Composition {
    serde_json::from_value(value).unwrap()
}
fn tool_loop() -> Composition {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node(
                "model",
                "agent",
                json!({"provider":"fixture","tools":["inspect_json"],"fanIn":"any","temperature":0.2,"maxOutputTokens":512}),
            ),
            node(
                "route",
                "condition",
                json!({"field":"hasToolCalls","equals":true}),
            ),
            node(
                "tools",
                "tool",
                json!({"tool":"execute_calls","ui":{"renderer":"table","title":"Résultats"}}),
            ),
            node("reply", "output", json!({"text":"{{output}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "model"),
            ("model", "route"),
            ("route", "tools"),
            ("route", "reply"),
            ("tools", "model"),
            ("reply", "e"),
        ],
    );
    value["edges"][2]["sourceHandle"] = json!("true");
    value["edges"][3]["sourceHandle"] = json!("false");
    value["settings"] = json!({"maxConcurrency":2,"strictChannels":true,"recursionLimit":20});
    parse(value)
}
#[tokio::test]
async fn model_tool_result_model_is_a_real_graph_loop() {
    let composition = tool_loop();
    let graph = materialize::build(&composition).unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("Bonjour"))]),
            ExecutionConfig::new("loop").with_recursion_limit(20),
        )
        .await
        .unwrap();
    assert_eq!(state["hasToolCalls"], false);
    assert_eq!(state["toolCalls"], json!([]));
    assert_eq!(state["toolResults"][0]["name"], "inspect_json");
    assert_eq!(state["toolResults"][0]["result"]["type"], "object");
    assert!(
        state["response"]
            .as_str()
            .unwrap()
            .contains("Outil inspect_json exécuté")
    );
    let history = state["messages"].as_array().unwrap();
    assert_eq!(
        history.len(),
        4,
        "user -> model function call -> function response -> model text"
    );
    assert_eq!(history[1]["parts"][0]["id"], history[2]["parts"][0]["id"]);
}
#[tokio::test]
async fn configured_state_reducers_preserve_their_semantics() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node("a", "set", json!({"field":"count","value":2})),
            node("b", "set", json!({"field":"count","value":3})),
            node("c", "set", json!({"field":"items","value":["new"]})),
            node("e", "end", json!({})),
        ],
        &[("s", "a"), ("a", "b"), ("b", "c"), ("c", "e")],
    );
    value["channels"] = json!([{"name":"count","reducer":"sum","default":10},{"name":"items","reducer":"append","default":["old"]}]);
    value["settings"] = json!({"strictChannels":true});
    let state = materialize::build(&parse(value))
        .unwrap()
        .invoke(State::new(), ExecutionConfig::new("reducers"))
        .await
        .unwrap();
    assert_eq!(state["count"], 15.0);
    assert_eq!(state["items"], json!(["old", "new"]));
}
#[tokio::test]
async fn timeout_and_retry_apply_to_actual_adk_tool_execution() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node(
                "slow",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":150}}),
            ),
            node("e", "end", json!({})),
        ],
        &[("s", "slow"), ("slow", "e")],
    );
    value["settings"] = json!({"timeoutMs":15,"retry":{"maxAttempts":2,"initialDelayMs":1,"maxDelayMs":1,"jitter":0,"retryOn":"timeout"}});
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_observed(&parse(value), Some(sender)).unwrap();
    let consumer = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(event) = receiver.recv().await {
            events.push(event);
        }
        events
    });
    let result = graph
        .invoke(State::new(), ExecutionConfig::new("timeout"))
        .await;
    drop(graph);
    assert!(result.unwrap_err().to_string().contains("timed out"));
    let mut starts = 0;
    for event in consumer.await.unwrap() {
        if event["type"] == "node_activity" && event["status"] == "running" {
            starts += 1;
        }
    }
    assert_eq!(
        starts, 2,
        "ADK should attempt the tool twice before failing"
    );
}
#[tokio::test]
async fn strict_channels_reject_undeclared_writes() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node("a", "set", json!({"field":"typo","value":2})),
            node("e", "end", json!({})),
        ],
        &[("s", "a"), ("a", "e")],
    );
    value["settings"] = json!({"strictChannels":true});
    let error = materialize::build(&parse(value))
        .unwrap()
        .invoke(State::new(), ExecutionConfig::new("strict"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("typo"), "{error}");
}
#[tokio::test]
async fn graph_concurrency_limits_admission_and_node_retry_overrides_defaults() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node(
                "a",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":20},"field":"aOut"}),
            ),
            node(
                "b",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":20},"field":"bOut"}),
            ),
            node("e", "end", json!({})),
        ],
        &[("s", "a"), ("s", "b"), ("a", "e"), ("b", "e")],
    );
    value["settings"] = json!({"maxConcurrency":1});
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_observed(&parse(value.clone()), Some(sender)).unwrap();
    graph
        .invoke(State::new(), ExecutionConfig::new("serial"))
        .await
        .unwrap();
    let mut active = 0;
    let mut maximum = 0;
    while let Ok(event) = receiver.try_recv() {
        if event["type"] != "node_activity" {
            continue;
        }
        if event["status"] == "running" {
            active += 1;
            maximum = maximum.max(active);
        } else {
            active -= 1;
        }
    }
    assert_eq!(maximum, 1, "ADK must admit only one graph node at a time");
    value["settings"] = json!({"maxConcurrency":1,"timeoutMs":2,"retry":{"maxAttempts":3,"initialDelayMs":1,"maxDelayMs":1,"retryOn":"timeout"}});
    value["nodes"][1]["data"]["config"]["retry"] = json!({"maxAttempts":1});
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_observed(&parse(value), Some(sender)).unwrap();
    assert!(
        graph
            .invoke(State::new(), ExecutionConfig::new("override"))
            .await
            .is_err()
    );
    let mut starts = 0;
    while let Ok(event) = receiver.try_recv() {
        if event["node"] == "a" && event["status"] == "running" {
            starts += 1;
        }
    }
    assert_eq!(
        starts, 1,
        "The per-node one-attempt policy overrides three default attempts"
    );
}

#[tokio::test]
async fn alternative_model_arrivals_stream_tools_then_wait_and_resume() {
    use futures::StreamExt;
    let mut value = serde_json::to_value(tool_loop()).unwrap();
    value["nodes"][1]["data"]["config"]["fanIn"] = json!("any");
    value["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|node| node["id"] != "e");
    value["nodes"].as_array_mut().unwrap().push(node(
        "ask",
        "input",
        json!({"field":"input","responseType":"text","prompt":"Continuer ?"}),
    ));
    value["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| edge["target"] != "e");
    value["edges"].as_array_mut().unwrap().extend([
        json!({"id":"reply-ask","source":"reply","target":"ask"}),
        json!({"id":"ask-model","source":"ask","target":"model"}),
    ]);
    let graph = materialize::build(&parse(value))
        .unwrap()
        .with_checkpointer(adk_graph::MemoryCheckpointer::new());
    let mut config = ExecutionConfig::new("interactive-tools").with_recursion_limit(80);
    let mut input = State::from([("input".into(), json!("Premier tour"))]);
    for (turn, expected) in [(1, "Premier tour"), (2, "Deuxième tour")] {
        let events = graph
            .stream(input, config, adk_graph::StreamMode::Debug)
            .collect::<Vec<_>>()
            .await;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, Ok(adk_graph::StreamEvent::Interrupted { .. }))),
            "A full model-tool-model loop must reach its input gate: {events:?}"
        );
        assert_eq!(events.iter().filter(|event|matches!(event,Ok(adk_graph::StreamEvent::NodeStart{node,..}) if node=="model")).count(),2);
        let checkpoint = graph
            .checkpointer()
            .unwrap()
            .load("interactive-tools")
            .await
            .unwrap()
            .unwrap();
        assert!(
            checkpoint.state["response"]
                .as_str()
                .unwrap()
                .contains(expected)
        );
        assert_eq!(
            checkpoint.state["messages"].as_array().unwrap().len(),
            turn * 4
        );
        config = ExecutionConfig::new("interactive-tools")
            .with_recursion_limit(80)
            .with_resume_from(&checkpoint.checkpoint_id);
        input = State::from([("answer:ask".into(), json!("Deuxième tour"))]);
    }
}

#[tokio::test]
async fn native_all_arrivals_wait_for_unequal_parallel_branches() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node("a", "set", json!({"field":"a","value":true})),
            node("b", "set", json!({"field":"b","value":true})),
            node("c", "set", json!({"field":"c","value":true})),
            node(
                "join",
                "set",
                json!({"field":"visits","value":1,"fanIn":"all"}),
            ),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "a"),
            ("s", "b"),
            ("b", "c"),
            ("a", "join"),
            ("c", "join"),
            ("join", "e"),
        ],
    );
    value["channels"] = json!([{"name":"visits","reducer":"sum","default":0}]);
    let state = materialize::build(&parse(value))
        .unwrap()
        .invoke(State::new(), ExecutionConfig::new("join"))
        .await
        .unwrap();
    assert_eq!(state["visits"], 1.0);
    assert_eq!(state["c"], true);
}

#[test]
fn runtime_primitives_reject_invalid_model_settings_and_unknown_tools() {
    let original = serde_json::to_value(tool_loop()).unwrap();
    for (pointer, invalid) in [
        ("/nodes/1/data/config/temperature", json!(5)),
        ("/nodes/1/data/config/tools", json!(["nonexistent-tool"])),
        ("/nodes/3/data/config/tool", json!("nonexistent-tool")),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            materialize::build(&parse(value)).is_err(),
            "accepted {pointer}"
        );
    }
}

#[tokio::test]
async fn predicate_routes_and_alternative_arrival_use_the_lowered_private_channel() {
    let mut value = composition(
        vec![
            node("s", "start", json!({})),
            node(
                "route",
                "condition",
                json!({"fanIn":"any", "predicate":{"kind":"compare","field":"input","operator":"eq","value":"first"}}),
            ),
            node("again", "set", json!({"field":"input","value":"second"})),
            node("reply", "output", json!({"text":"{{input}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "route"),
            ("route", "again"),
            ("route", "reply"),
            ("again", "route"),
            ("reply", "e"),
        ],
    );
    value["formatVersion"] = json!(2);
    value["settings"] = json!({"strictChannels":true});
    value["edges"][1]["sourceHandle"] = json!("true");
    value["edges"][2]["sourceHandle"] = json!("false");
    let graph = materialize::build(&parse(value)).unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("first"))]),
            ExecutionConfig::new("predicate-loop"),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "second");
    assert_eq!(state["__zedflow:condition:route"], false);
}

#[tokio::test]
async fn planned_subgraphs_keep_isolated_inputs_outputs_and_scoped_observation() {
    let child = parse(composition(
        vec![
            node("s", "start", json!({})),
            node("out", "output", json!({"text":"Child: {{input}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "out"), ("out", "e")],
    ));
    let parent = parse(composition(
        vec![
            node("s", "start", json!({})),
            node("child", "subgraph", json!({"composition":child})),
            node("out", "output", json!({"text":"Parent: {{output}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "child"), ("child", "out"), ("out", "e")],
    ));
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_observed(&parent, Some(sender)).unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("question"))]),
            ExecutionConfig::new("nested"),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "Parent: Child: question");
    let mut paths = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        if event["status"] == "completed" {
            paths.push(event["path"].clone());
        }
    }
    assert!(paths.contains(&json!("child/out")), "{paths:?}");
    assert!(paths.contains(&json!("child")), "{paths:?}");
}

#[tokio::test]
async fn native_nodes_receive_the_real_adk_context_through_observation() {
    use adk_graph::{END, NodeOutput, START, StateGraph, StateSchema};
    let doc = parse(composition(
        vec![
            node("s", "start", json!({})),
            node(
                "native",
                "set",
                json!({"field":"output","value":"must not run"}),
            ),
            node("e", "end", json!({})),
        ],
        &[("s", "native"), ("native", "e")],
    ));
    let native = StateGraph::new(StateSchema::simple(&["input", "output"]))
        .add_node_fn("native", |ctx| async move {
            // Rebuilding NodeContext from state/config/step would lose this
            // executor-owned schema. The observation wrappers must delegate it.
            assert!(ctx.parent_schema().is_some());
            assert_eq!(ctx.config.thread_id, "native-context");
            assert_eq!(ctx.config.metadata["token"], "retained");
            Ok(NodeOutput::new().with_update("output", ctx.state["input"].clone()))
        })
        .add_edge(START, "native")
        .add_edge("native", END)
        .compile()
        .unwrap();
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_scope_with_native(
        &doc,
        Some(sender),
        "root/",
        None,
        None,
        Some(&native),
    )
    .unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("native result"))]),
            ExecutionConfig::new("native-context").with_metadata("token", json!("retained")),
        )
        .await
        .unwrap();
    assert_eq!(state["output"], "native result");
    let mut running = 0;
    let mut completed = 0;
    while let Ok(event) = receiver.try_recv() {
        assert_eq!(event["path"], "root/native");
        if event["status"] == "running" {
            running += 1;
        }
        if event["status"] == "completed" {
            completed += 1;
        }
    }
    assert_eq!((running, completed), (1, 1));
}

#[tokio::test]
async fn compiled_snapshot_materializes_its_frozen_instance() {
    use zf_compiler::{
        compiler::{CompileRequest, compile},
        graph_compiler::GraphValidator,
        prepared::CompilationSnapshot,
        programs::SourceSnapshot,
    };
    use zf_flows::composition::ResolveRequest;
    let mut value = composition(
        vec![
            node(
                "s",
                "start",
                json!({"exports":{
                    "contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}},"secondary":{"input":{"kind":"text"},"output":{"kind":"text"}}}},
                    "entries":{"main":{"node":"s","inputField":"input","outputField":"response"},"secondary":{"node":"out","inputField":"input","outputField":"response"}},"interactive":false
                }}),
            ),
            node(
                "skip",
                "set",
                json!({"field":"input","value":"authored start"}),
            ),
            node("out", "output", json!({"text":"Frozen: {{input}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "skip"), ("skip", "out"), ("out", "e")],
    );
    value["formatVersion"] = json!(2);
    let doc = parse(value);
    let validator = GraphValidator::new(&materialize::RuntimePrimitives);
    let mut snapshot = CompilationSnapshot::default();
    snapshot.flows.insert(
        "fixture".into(),
        SourceSnapshot::capture(zf_flows::flow_format::render(&doc, &validator).unwrap()),
    );
    let request = CompileRequest::new(ResolveRequest {
        flow: "fixture".into(),
        entry: "secondary".into(),
        bridges: vec![],
    });
    let compiled = compile(&snapshot, &request, &materialize::RuntimePrimitives).unwrap();
    let graph = materialize::build_compiled(&compiled, "root", None, None, None).unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("question"))]),
            ExecutionConfig::new("frozen"),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "Frozen: question");
    let explicit =
        materialize::build_compiled_entry(&compiled, "root", "main", None, None, None).unwrap();
    let state = explicit
        .invoke(
            State::from([("input".into(), json!("question"))]),
            ExecutionConfig::new("explicit"),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "Frozen: authored start");
    assert!(
        materialize::build_compiled_entry(&compiled, "root", "missing", None, None, None).is_err()
    );
    assert!(materialize::build_compiled(&compiled, "missing", None, None, None).is_err());
}

#[tokio::test]
async fn revision_preparation_preserves_native_context_and_the_pinned_step_after_resume() {
    use adk_graph::{END, NodeOutput, START, StateGraph, StateSchema, checkpoint::Checkpointer};
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeMap, sync::Arc};
    use zf_runtime::{
        revisions::{Compatibility, RevisionDefinition, RevisionRuntime},
        stored_checkpointer::StoredCheckpointer,
    };
    use zf_storage::{content_store::ContentStore, contracts::CheckpointStore};

    fn definition(doc: &Composition) -> RevisionDefinition {
        let source = zf_flows::flow_format::render(
            doc,
            &zf_compiler::graph_compiler::GraphValidator::new(&materialize::RuntimePrimitives),
        )
        .unwrap();
        RevisionDefinition {
            key: "fixture".into(),
            hash: format!("{:x}", Sha256::digest(source.as_bytes())),
            source,
            composition: doc.clone(),
        }
    }

    let temp = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("revisions.db"))
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
        )
        .await
        .unwrap();
    let content = ContentStore::new(pool).await.unwrap();
    let checkpoints = Arc::new(StoredCheckpointer::new(
        CheckpointStore::new(content.clone()).await.unwrap(),
    ));
    let doc = parse(composition(
        vec![
            node("s", "start", json!({})),
            node(
                "native",
                "set",
                json!({"field":"output","value":"document baseline"}),
            ),
            node("e", "end", json!({})),
        ],
        &[("s", "native"), ("native", "e")],
    ));
    let base = definition(&doc);
    let baseline_hash = base.hash.clone();
    let revisions = RevisionRuntime::new(
        content.clone(),
        "revision-run",
        BTreeMap::from([("root".into(), base.clone())]),
    )
    .await
    .unwrap();
    let native = StateGraph::new(StateSchema::simple(&["input", "output"]))
        .add_node_fn("native", move |ctx| {
            let baseline_hash = baseline_hash.clone();
            async move {
                assert!(
                    ctx.parent_schema().is_some(),
                    "revision wrappers lost the ADK context"
                );
                assert_eq!(
                    zf_runtime::revisions::current_revision().unwrap()["hash"],
                    baseline_hash
                );
                assert_eq!(ctx.config.metadata["identity"], "preserved");
                if ctx.state.get("input") != Some(&json!("resumed input")) {
                    return Ok(NodeOutput::new().with_interrupt(
                        adk_graph::interrupt::interrupt_with_data(
                            "wait before completing native node",
                            json!({"native":true}),
                        ),
                    ));
                }
                Ok(NodeOutput::new().with_update("output", "pinned native result"))
            }
        })
        .add_edge(START, "native")
        .add_edge("native", END)
        .compile()
        .unwrap();
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    let graph = materialize::build_scope_with_native_and_revisions(
        &doc,
        Some(sender.clone()),
        "root/",
        None,
        Some(checkpoints.clone()),
        Some(&native),
        Some(revisions.clone()),
    )
    .unwrap();
    let invocation =
        || ExecutionConfig::new("revision-thread").with_metadata("identity", json!("preserved"));
    assert!(matches!(
        graph.invoke(State::new(), invocation()).await.unwrap_err(),
        adk_graph::error::GraphError::Interrupted(_)
    ));
    let paused = checkpoints.load("revision-thread").await.unwrap().unwrap();
    let mut changed = doc.clone();
    changed.nodes[1].data.config["value"] = json!("published result");
    assert_eq!(
        revisions
            .publish("root", definition(&changed))
            .await
            .unwrap(),
        Compatibility::Live
    );
    drop(graph);
    drop(revisions);
    // Recreate both revision controller and graph against durable records. The
    // pending super-step must retain its previous definition and native node.
    let revisions = RevisionRuntime::new(
        content,
        "revision-run",
        BTreeMap::from([("root".into(), base)]),
    )
    .await
    .unwrap();
    let graph = materialize::build_scope_with_native_and_revisions(
        &doc,
        Some(sender),
        "root/",
        None,
        Some(checkpoints),
        Some(&native),
        Some(revisions),
    )
    .unwrap();
    let state = graph
        .invoke(
            State::from([("input".into(), json!("resumed input"))]),
            invocation().with_resume_from(&paused.checkpoint_id),
        )
        .await
        .unwrap();
    assert_eq!(state["output"], "pinned native result");
    let next = graph
        .invoke(State::new(), ExecutionConfig::new("next-revision-thread"))
        .await
        .unwrap();
    assert_eq!(
        next["output"], "published result",
        "a fresh step must adopt the published plan node"
    );
    let mut running = 0;
    let mut terminal = 0;
    while let Ok(event) = receiver.try_recv() {
        if event["type"] != "node_activity" {
            continue;
        }
        assert_eq!(event["path"], "root/native");
        if event["status"] == "running" {
            running += 1;
        } else {
            terminal += 1;
        }
    }
    assert_eq!(
        (running, terminal),
        (3, 3),
        "preparation and execution must share one observation attempt"
    );
}

#[tokio::test]
async fn resumable_nested_graph_preserves_child_state_targeted_answers_and_effect_receipts() {
    use adk_graph::checkpoint::Checkpointer;
    use std::sync::Arc;
    use zf_runtime::{runtime::RunServices, stored_checkpointer::StoredCheckpointer};
    use zf_storage::{content_store::ContentStore, contracts::CheckpointStore};

    fn wrap(child: Composition, id: &str) -> Composition {
        parse(composition(
            vec![
                node("s", "start", json!({})),
                node(id, "subgraph", json!({"composition":child})),
                node("out", "output", json!({"text":"{{output}}"})),
                node("e", "end", json!({})),
            ],
            &[("s", id), (id, "out"), ("out", "e")],
        ))
    }
    async fn adapters(
        root: &std::path::Path,
    ) -> (ContentStore, Arc<StoredCheckpointer>, Arc<RunServices>) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.join("nested.db"))
                    .create_if_missing(true)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
            )
            .await
            .unwrap();
        let content = ContentStore::new(pool).await.unwrap();
        let checkpoints = Arc::new(StoredCheckpointer::new(
            CheckpointStore::new(content.clone()).await.unwrap(),
        ));
        let context = zf_runtime::workspace_context::ContextSnapshot::load_with_home(
            root,
            &[],
            Some(&root.join("fixture-home")),
        )
        .await
        .unwrap();
        let services = RunServices::new(
            "nested-run".into(),
            root.into(),
            root.join("data"),
            context,
            json!({}),
            vec![],
        )
        .unwrap();
        services.set_content_store(content.clone());
        (content, checkpoints, services)
    }
    let temp = tempfile::tempdir().unwrap();
    let child = parse(composition(
        vec![
            node("s", "start", json!({})),
            node(
                "change",
                "set",
                json!({"field":"input","value":"{{input}}-child"}),
            ),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
            node(
                "ask",
                "input",
                json!({"field":"answer","responseType":"text","prompt":"Continue child?"}),
            ),
            node("out", "output", json!({"text":"{{input}}/{{answer}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "change"),
            ("change", "effect"),
            ("effect", "ask"),
            ("ask", "out"),
            ("out", "e"),
        ],
    ));
    let doc = wrap(wrap(child, "inner"), "outer");
    let (content, checkpoints, services) = adapters(temp.path()).await;
    let graph =
        materialize::build_with_services(&doc, services, None, Some(checkpoints.clone())).unwrap();
    let error = graph
        .invoke(
            State::from([("input".into(), json!("original"))]),
            ExecutionConfig::new("nested-thread"),
        )
        .await
        .unwrap_err();
    let adk_graph::error::GraphError::Interrupted(pause) = error else {
        panic!("expected nested wait: {error}")
    };
    let adk_graph::Interrupt::Dynamic {
        data: Some(mut wait),
        ..
    } = pause.interrupt
    else {
        panic!("expected dynamic nested wait")
    };
    while wait.get("nodePath").is_none() {
        wait = wait.get("data").expect("nested wait payload").clone();
    }
    assert_eq!(wait["nodePath"], "outer/inner/ask");
    assert_eq!(
        tokio::fs::read_to_string(temp.path().join("visits"))
            .await
            .unwrap(),
        "x"
    );
    let receipts_before: Vec<_> = content
        .records("nested-run")
        .await
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == "receipts")
        .collect();
    assert_eq!(receipts_before.len(), 1);
    let saved = checkpoints.load("nested-thread").await.unwrap().unwrap();
    drop(graph);
    drop(checkpoints);
    content.pool().close().await;
    drop(content);

    let (content, checkpoints, services) = adapters(temp.path()).await;
    let graph = materialize::build_with_services(&doc, services, None, Some(checkpoints)).unwrap();
    let state = graph
        .invoke(
            State::from([
                (
                    "input".into(),
                    json!("new parent input must not reset child"),
                ),
                (
                    "answer:outer/inner/ask".into(),
                    json!({"__zedflowAnswerId":"nested-answer-1","value":"done"}),
                ),
            ]),
            ExecutionConfig::new("nested-thread").with_resume_from(&saved.checkpoint_id),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "original-child/done");
    assert_eq!(
        tokio::fs::read_to_string(temp.path().join("visits"))
            .await
            .unwrap(),
        "x"
    );
    let receipts_after: Vec<_> = content
        .records("nested-run")
        .await
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == "receipts")
        .collect();
    assert_eq!(
        receipts_after.len(),
        1,
        "resuming the child must retain the original effect receipt"
    );
    assert_eq!(receipts_before[0].value_ref, receipts_after[0].value_ref);
}
