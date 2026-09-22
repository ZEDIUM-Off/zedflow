use adk_graph::{ExecutionConfig, State};
use serde_json::{Value, json};
use zf_flows::schema::Composition;

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
    let graph = zf_runtime::materialize::build(&composition).unwrap();
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
    let state = zf_runtime::materialize::build(&parse(value))
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
    let graph = zf_runtime::materialize::build_observed(&parse(value), Some(sender)).unwrap();
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
    let error = zf_runtime::materialize::build(&parse(value))
        .unwrap()
        .invoke(State::new(), ExecutionConfig::new("strict"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("typo"), "{error}");
}
#[test]
fn invalid_parameters_are_rejected_before_codegen_or_execution() {
    let valid = serde_json::to_value(tool_loop()).unwrap();
    for (pointer, value) in [
        ("/settings/maxConcurrency", json!(0)),
        ("/settings/recursionLimit", json!(0)),
        ("/settings/timeoutMs", json!(0)),
        ("/nodes/1/data/config/temperature", json!(5)),
        ("/nodes/1/data/config/tools", json!(["shell"])),
        ("/nodes/3/data/config/ui/renderer", json!("javascript")),
    ] {
        let mut broken = valid.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        broken.pointer_mut(parent).unwrap()[key] = value;
        assert!(
            zf_compiler::graph_compiler::validate(
                &parse(broken),
                &zf_runtime::materialize::RuntimePrimitives
            )
            .is_err(),
            "Invalid {pointer} should not be ignored"
        );
    }
}
#[tokio::test]
async fn generated_model_tool_loop_matches_daemon() {
    if std::env::var_os("ZEDFLOW_TEST_CODEGEN").is_none() {
        return;
    }
    let composition = tool_loop();
    let directory = tempfile::tempdir().unwrap();
    let home = directory.path().join(".fixture-home");
    std::fs::create_dir_all(home.join(".pi/agent")).unwrap();
    std::fs::write(
        home.join(".pi/agent/AGENTS.md"),
        "portable-parity-fixture-guidance",
    )
    .unwrap();
    let context = zf_runtime::workspace_context::ContextSnapshot::load_with_home(
        directory.path(),
        &[],
        Some(&home),
    )
    .await
    .unwrap();
    assert_eq!(context.instructions.len(), 1);
    assert!(
        context
            .instructions
            .iter()
            .all(|instruction| instruction.path.starts_with(&home))
    );
    let services = zf_runtime::runtime::RunServices::new(
        "parity".into(),
        directory.path().into(),
        directory.path().join("expected-data"),
        context.clone(),
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_context_sources(vec![], Some(home.clone()));
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let expected_store = zf_storage::content_store::ContentStore::new(db)
        .await
        .unwrap();
    services.set_content_store(expected_store.clone());
    let (sender, mut receiver) = zf_runtime::event_sink::channel(1024);
    services.set_sender(Some(sender.clone()));
    let mut expected =
        zf_runtime::materialize::build_with_services(&composition, services, Some(sender), None)
            .unwrap()
            .invoke(
                State::from([("input".into(), json!("Bonjour"))]),
                ExecutionConfig::new("parity").with_recursion_limit(20),
            )
            .await
            .unwrap();
    let mut expected_events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        expected_events.push(event);
    }
    let generated = zf_compiler::export::export_single(
        &composition,
        &zf_flows::flow_source::render(
            &composition,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
        None,
        &zf_runtime::materialize::RuntimePrimitives,
        &zf_runtime::runtime_export::support(),
    )
    .unwrap();
    for (relative, content) in &generated.files {
        let path = directory.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let output = tokio::process::Command::new("cargo")
        .args(["run", "--quiet", "--manifest-path"])
        .arg(directory.path().join("Cargo.toml"))
        .args(["--", "--workspace"])
        .arg(directory.path())
        .arg("--home")
        .arg(directory.path().join(".fixture-home"))
        .arg("--data")
        .arg(directory.path().join("data"))
        .args(["--run-id", "parity", "--input", r#"{"input":"Bonjour"}"#])
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(outcome["status"], "completed", "{outcome}");
    let mut actual: State = serde_json::from_value(outcome["state"].clone()).unwrap();
    let export_db = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join("data/sessions.db"))
            .read_only(true),
    )
    .await
    .unwrap();
    let export_store = zf_storage::content_store::ContentStore::new(export_db)
        .await
        .unwrap();
    let export_context: Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("data/parity/context.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(export_context, serde_json::to_value(&context).unwrap());
    let export_events: Vec<Value> =
        std::fs::read_to_string(directory.path().join("data/parity/events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
    for (state, store, events) in [
        (&mut actual, &export_store, &export_events),
        (&mut expected, &expected_store, &expected_events),
    ] {
        let response = state.get_mut("modelResponse").unwrap();
        let invocation = response["invocationId"].as_str().unwrap();
        uuid::Uuid::parse_str(invocation).unwrap();
        assert_eq!(response["nodePath"], "model");
        assert_eq!(response["selection"]["provider"], "fixture");
        let mut manifest = store
            .resolve(response["requestRef"].as_str().unwrap())
            .await
            .unwrap();
        assert_eq!(manifest["invocationId"], invocation);
        assert_eq!(manifest["nodePath"], response["nodePath"]);
        assert_eq!(manifest["selection"], response["selection"]);
        let event = events
            .iter()
            .find(|event| event["type"] == "model_request" && event["invocationId"] == invocation)
            .expect("real model request event");
        assert_eq!(event["requestRef"], response["requestRef"]);
        assert_eq!(event["nodePath"], "model");
        assert_eq!(event["origin"], manifest["origin"]);
        assert_eq!(manifest["origin"]["nodePath"], "model");
        let occurrence = manifest["origin"]["occurrenceId"].as_str().unwrap();
        uuid::Uuid::parse_str(occurrence).unwrap();
        assert!(events.iter().any(|event| event["type"] == "node_activity"
            && event["nodePath"] == "model"
            && event["occurrenceId"] == occurrence));
        assert!(
            manifest["request"]["contents"][0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("portable-parity-fixture-guidance")
        );
        manifest["origin"]["occurrenceId"] = json!("independent-observed-passage");
        manifest["invocationId"] = json!("independent-invocation");
        response["invocationId"] = json!("independent-invocation");
        response["requestRef"] = manifest;
    }
    assert_eq!(actual, expected);
    for field in ["nodePath", "occurrenceId"] {
        let mut changed_origin = actual.clone();
        changed_origin.get_mut("modelResponse").unwrap()["requestRef"]["origin"][field] =
            json!("different-origin");
        assert_ne!(
            changed_origin, expected,
            "origin remains part of complete parity"
        );
    }
    let mut changed = actual.clone();
    changed.get_mut("modelResponse").unwrap()["requestRef"]["request"]["changedParameter"] =
        json!(true);
    assert_ne!(
        changed, expected,
        "request content remains part of complete parity"
    );
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
    let graph =
        zf_runtime::materialize::build_observed(&parse(value.clone()), Some(sender)).unwrap();
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
    let graph = zf_runtime::materialize::build_observed(&parse(value), Some(sender)).unwrap();
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
    let graph = zf_runtime::materialize::build(&parse(value))
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
    let state = zf_runtime::materialize::build(&parse(value))
        .unwrap()
        .invoke(State::new(), ExecutionConfig::new("join"))
        .await
        .unwrap();
    assert_eq!(state["visits"], 1.0);
    assert_eq!(state["c"], true);
}

#[test]
fn custom_model_history_cannot_accumulate_full_history_again() {
    let mut value = serde_json::to_value(tool_loop()).unwrap();
    value["nodes"][1]["data"]["config"]["historyField"] = json!("conversation");
    value["channels"] = json!([{"name":"conversation","reducer":"append"}]);
    assert!(
        zf_compiler::graph_compiler::validate(
            &parse(value),
            &zf_runtime::materialize::RuntimePrimitives
        )
        .unwrap_err()
        .to_string()
        .contains("overwrite")
    );
}

#[test]
fn template_values_are_never_reinterpreted() {
    let state = State::from([
        ("input".into(), json!("{{private}}")),
        ("private".into(), json!("not-for-this-output")),
    ]);
    assert_eq!(
        zf_runtime::operations::render("Entrée : {{input}} / {{missing}}", &state),
        "Entrée : {{private}} / {{missing}}"
    );
}
