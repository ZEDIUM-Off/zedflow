use adk_graph::prelude::*;
use futures::StreamExt;
use serde_json::{Value, json};
use std::sync::Arc;
use zf_context::context::ContextBlock;
use zf_context::context::ContextExpr;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_context::context_source;
use zf_core::types::DataType;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
use zf_runtime::models;
use zf_runtime::runtime::RunServices;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::context_store;

fn context_config() -> Value {
    let strategy = ContextStrategy::new("explicit", "Explicit preparation")
        .require("input", DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "question",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("input"),
        )]);
    let source = context_source::generate(&strategy).unwrap();
    json!({"modelNode":"model","contextProgram":{"strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),"types":{},"bindings":{"input":{"kind":"state","field":"input"}}},"fanIn":"any"})
}
fn model_config() -> Value {
    json!({"contextNode":"context","provider":"fixture","fixtureSteps":[{"echoRequest":true}],"field":"output"})
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"type":"custom","position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn document() -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":"pair","name":"Pair","revision":0,
        "nodes":[node("start","start",json!({})),node("context","context",context_config()),node("model","model",model_config()),node("wait","input",json!({"field":"input","responseType":"text","prompt":"Continue?"}))],
        "edges":[{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"wait"},{"id":"d","source":"wait","target":"context"}]
    })).unwrap()
}
fn services(root: &std::path::Path) -> Arc<RunServices> {
    RunServices::new(
        "context-pair".into(),
        root.into(),
        root.join("data"),
        ContextSnapshot::default(),
        json!({}),
        vec![],
    )
    .unwrap()
}
fn runtime_config(mut config: Value, id: &str) -> Value {
    config["__zedflowVersion"] = json!(3);
    config["nodeId"] = json!(id);
    config
}

#[test]
fn explicit_context_model_source_roundtrips_and_every_arrival_is_checked() {
    let doc = document();
    zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
    let source = flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert!(source.contains("models::context_node_with_services"));
    assert!(source.contains("models::inference_node_with_services"));
    assert_eq!(
        serde_json::to_value(
            flow_source::parse(
                &source,
                &zf_compiler::graph_compiler::GraphValidator::new(
                    &zf_runtime::materialize::RuntimePrimitives
                )
            )
            .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&doc).unwrap()
    );
    let mut bypass = serde_json::to_value(&doc).unwrap();
    bypass["edges"][3]["target"] = json!("model");
    let error = zf_compiler::graph_compiler::validate(
        &serde_json::from_value(bypass).unwrap(),
        &zf_runtime::materialize::RuntimePrimitives,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("chaque passage"), "{error}");
    let mut incompatible = serde_json::to_value(&doc).unwrap();
    incompatible["nodes"][2]["data"]["config"]["tools"] = json!(["exec"]);
    assert!(
        zf_compiler::graph_compiler::validate(
            &serde_json::from_value(incompatible).unwrap(),
            &zf_runtime::materialize::RuntimePrimitives
        )
        .is_err()
    );
}

#[tokio::test]
async fn model_consumes_exact_preparation_once_without_reacquiring_mutated_state() {
    let root = tempfile::tempdir().unwrap();
    let services = services(root.path());
    let context_cfg = runtime_config(context_config(), "context");
    let model_cfg = runtime_config(model_config(), "model");
    let preparation = models::context_node_with_services(
        "context",
        &context_cfg,
        &model_cfg,
        "context",
        services.clone(),
    )
    .unwrap();
    let model = models::inference_node_with_services(
        "model",
        &model_cfg,
        &context_cfg,
        "model",
        services.clone(),
    )
    .unwrap();
    let state = State::from([("input".into(), json!("captured before model"))]);
    let absent = model
        .execute(&NodeContext::new(
            state.clone(),
            ExecutionConfig::new("context-pair"),
            0,
        ))
        .await
        .err()
        .unwrap();
    assert!(absent.to_string().contains("précédé"));
    let prepared = preparation
        .execute(&NodeContext::new(
            state.clone(),
            ExecutionConfig::new("context-pair"),
            1,
        ))
        .await
        .unwrap();
    assert!(prepared.updates["__zedflow:prepared:context"].is_string());
    assert!(
        !serde_json::to_string(&prepared.updates)
            .unwrap()
            .contains("captured before model")
    );
    let mut state = state;
    state.extend(prepared.updates);
    state.insert("input".into(), json!("changed after preparation"));
    let result = model
        .execute(&NodeContext::new(
            state.clone(),
            ExecutionConfig::new("context-pair"),
            2,
        ))
        .await
        .unwrap();
    let request: Value = serde_json::from_str(result.updates["output"].as_str().unwrap()).unwrap();
    assert_eq!(
        request["contents"][0]["parts"][0]["text"],
        "captured before model"
    );
    assert_eq!(
        result.updates["modelResponse"]["preparationId"],
        state["__zedflow:prepared:context"]
    );
    state.extend(result.updates);
    let duplicate = model
        .execute(&NodeContext::new(
            state,
            ExecutionConfig::new("context-pair"),
            3,
        ))
        .await
        .err()
        .unwrap();
    assert!(duplicate.to_string().contains("déjà été consommée"));
}

#[tokio::test]
async fn interactive_loop_reprepares_on_resume_and_observes_two_real_nodes() {
    let root = tempfile::tempdir().unwrap();
    let services = services(root.path());
    let checkpoint = Arc::new(adk_graph::checkpoint::MemoryCheckpointer::new());
    let doc = document();
    let graph = zf_runtime::materialize::build_with_services(
        &doc,
        services.clone(),
        None,
        Some(checkpoint.clone()),
    )
    .unwrap();
    let mut config = ExecutionConfig::new("context-pair");
    let mut input = State::from([("input".into(), json!("first turn"))]);
    let mut previous = Value::Null;
    for text in ["first turn", "second turn"] {
        let events = graph
            .stream(input, config, adk_graph::StreamMode::Debug)
            .collect::<Vec<_>>()
            .await;
        assert!(events.iter().all(Result::is_ok), "{events:?}");
        for expected in ["context", "model"] {
            assert_eq!(events.iter().filter(|event| matches!(event,Ok(adk_graph::StreamEvent::NodeStart { node,.. }) if node==expected)).count(), 1, "{events:?}");
        }
        let saved = graph
            .checkpointer()
            .unwrap()
            .load("context-pair")
            .await
            .unwrap()
            .unwrap();
        let request: Value = serde_json::from_str(saved.state["output"].as_str().unwrap()).unwrap();
        assert_eq!(request["contents"][0]["parts"][0]["text"], text);
        assert_ne!(saved.state["__zedflow:prepared:context"], previous);
        previous = saved.state["__zedflow:prepared:context"].clone();
        config = ExecutionConfig::new("context-pair").with_resume_from(&saved.checkpoint_id);
        input = State::from([("answer:wait".into(), json!("second turn"))]);
    }
}

#[tokio::test]
async fn projected_context_is_never_appended_to_canonical_conversation() {
    use std::collections::BTreeMap;
    let root = tempfile::tempdir().unwrap();
    let services = services(root.path());
    let strategy = ContextStrategy::new("conversation", "Conversation with projection")
        .require(
            "history",
            DataType::List {
                item: Box::new(DataType::Record {
                    fields: BTreeMap::new(),
                }),
            },
        )
        .with_program(vec![
            ContextBlock::emit(
                "guide",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!("EXTERNAL FILE PROJECTION")),
            ),
            ContextBlock::emit(
                "history",
                FragmentRole::Data,
                FragmentFormat::AdkMessages,
                ContextExpr::resource("history"),
            ),
        ]);
    let source = context_source::generate(&strategy).unwrap();
    let context = runtime_config(
        json!({"modelNode":"model","contextProgram":{"strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),"types":{},"bindings":{"history":{"kind":"conversation","historyField":"messages","inputField":"input"}}}}),
        "context",
    );
    let mut model_cfg = model_config();
    // A fixed answer avoids echoing the projected data as assistant output.
    model_cfg["fixtureSteps"] = json!([{"text":"done"}]);
    let model_cfg = runtime_config(model_cfg, "model");
    let context_node = models::context_node_with_services(
        "context",
        &context,
        &model_cfg,
        "context",
        services.clone(),
    )
    .unwrap();
    let model = models::inference_node_with_services(
        "model",
        &model_cfg,
        &context,
        "model",
        services.clone(),
    )
    .unwrap();
    let mut state = State::new();
    for turn in 0..3 {
        state.insert("input".into(), json!(format!("request {turn}")));
        state.insert("__zedflow:input:input".into(), json!(turn));
        let prepared = context_node
            .execute(&NodeContext::new(
                state.clone(),
                ExecutionConfig::new("context-pair"),
                turn * 2,
            ))
            .await
            .unwrap();
        state.extend(prepared.updates);
        let output = model
            .execute(&NodeContext::new(
                state.clone(),
                ExecutionConfig::new("context-pair"),
                turn * 2 + 1,
            ))
            .await
            .unwrap();
        let id = output.updates["modelResponse"]["invocationId"]
            .as_str()
            .unwrap();
        let request = services
            .read_record("model-requests", id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            request["request"]["contents"].as_array().unwrap().len(),
            turn * 2 + 2
        );
        assert_eq!(
            request
                .to_string()
                .matches("EXTERNAL FILE PROJECTION")
                .count(),
            1
        );
        let history = output.updates["messages"].as_array().unwrap();
        assert_eq!(history.len(), turn * 2 + 2);
        assert!(
            !serde_json::to_string(history)
                .unwrap()
                .contains("EXTERNAL FILE PROJECTION")
        );
        state.extend(output.updates);
    }
}

#[tokio::test]
async fn preparation_is_restored_from_durable_records_without_running_context_again() {
    let root = tempfile::tempdir().unwrap();
    let original = services(root.path());
    let context_cfg = runtime_config(context_config(), "context");
    let model_cfg = runtime_config(model_config(), "model");
    let context = models::context_node_with_services(
        "context",
        &context_cfg,
        &model_cfg,
        "context",
        original.clone(),
    )
    .unwrap();
    let output = context
        .execute(&NodeContext::new(
            State::from([("input".into(), json!("durable preparation"))]),
            ExecutionConfig::new("context-pair"),
            0,
        ))
        .await
        .unwrap();
    let checkpoint_bytes = serde_json::to_vec(&output.updates).unwrap();
    drop(context);
    drop(original);
    let restored = services(root.path());
    let model =
        models::inference_node_with_services("model", &model_cfg, &context_cfg, "model", restored)
            .unwrap();
    let restored_state = serde_json::from_slice(&checkpoint_bytes).unwrap();
    let result = model
        .execute(&NodeContext::new(
            restored_state,
            ExecutionConfig::new("context-pair"),
            1,
        ))
        .await
        .unwrap();
    assert!(
        result.updates["output"]
            .as_str()
            .unwrap()
            .contains("durable preparation")
    );
}

#[tokio::test]
async fn exact_exported_context_model_pair_executes_with_same_prepared_input() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let mut value = serde_json::to_value(document()).unwrap();
    value["nodes"][3] = node("end", "end", json!({}));
    value["edges"][2]["target"] = json!("end");
    value["edges"].as_array_mut().unwrap().pop();
    value["settings"]["workingDirectory"] = json!("docs");
    value["nodes"][1]["data"]["config"]["contextProgram"]["bindings"]["input"] = json!({"kind":"reader","reader":"file.text","input":{"kind":"literal","value":{"path":"question.txt"}}});
    let doc: Composition = serde_json::from_value(value).unwrap();
    let source = flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert_eq!(
        flow_source::parse(
            &source,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives
            )
        )
        .unwrap()
        .settings
        .working_directory
        .as_deref(),
        Some("docs")
    );
    let export = zf_compiler::export::export_single(
        &doc,
        &zf_flows::flow_source::render(
            &doc,
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
    let root = tempfile::tempdir().unwrap();
    for (relative, content) in &export.files {
        let path = root.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("docs")).unwrap();
    std::fs::write(
        workspace.path().join("docs/question.txt"),
        "portable exact preparation",
    )
    .unwrap();
    let expected =
        zf_runtime::materialize::build_with_services(&doc, services(workspace.path()), None, None)
            .unwrap()
            .invoke(State::new(), ExecutionConfig::new("context-pair"))
            .await
            .unwrap();
    let expected: Value = serde_json::from_str(expected["output"].as_str().unwrap()).unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["run", "--quiet", "--offline", "--manifest-path"])
            .arg(root.path().join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(workspace.path())
            .arg("--home")
            .arg(workspace.path().join(".fixture-home"))
            .arg("--data")
            .arg(workspace.path().join("data"))
            .args([
                "--run-id",
                "portable-pair",
                "--input",
                r#"{"input":"portable exact preparation"}"#,
            ])
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .unwrap_or_else(|| "/tmp/zedflow-adk-target".into()),
            )
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let state: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(state["status"], "completed", "{state}");
    let state = &state["state"];
    let request: Value = serde_json::from_str(state["output"].as_str().unwrap()).unwrap();
    assert_eq!(request, expected);
    assert_eq!(
        request["contents"][0]["parts"][0]["text"],
        "portable exact preparation"
    );
}

#[tokio::test]
async fn changing_runtime_model_preferences_does_not_retarget_an_existing_preparation() {
    let root = tempfile::tempdir().unwrap();
    let original = services(root.path());
    original.set_binding(
        "model".into(),
        json!({"provider":"fixture","model":"prepared-model"}),
    );
    let context_cfg = runtime_config(context_config(), "context");
    let mut model_cfg = runtime_config(model_config(), "model");
    model_cfg["modelBinding"] = json!("runtime");
    let context = models::context_node_with_services(
        "context",
        &context_cfg,
        &model_cfg,
        "context",
        original.clone(),
    )
    .unwrap();
    let output = context
        .execute(&NodeContext::new(
            State::from([("input".into(), json!("prepared with fixture"))]),
            ExecutionConfig::new("context-pair"),
            0,
        ))
        .await
        .unwrap();
    drop(context);
    drop(original);
    let restored = services(root.path());
    // A different preference after preparation must not initialize that provider
    // or reinterpret its already adapted context. No live model is invoked.
    restored.set_binding(
        "model".into(),
        json!({"provider":"gemini","model":"next-model"}),
    );
    let model =
        models::inference_node_with_services("model", &model_cfg, &context_cfg, "model", restored)
            .unwrap();
    let result = model
        .execute(&NodeContext::new(
            output.updates,
            ExecutionConfig::new("context-pair"),
            1,
        ))
        .await
        .unwrap();
    assert_eq!(result.updates["modelResponse"]["provider"], "fixture");
}
