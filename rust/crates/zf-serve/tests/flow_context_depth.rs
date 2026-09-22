use serde_json::{Value, json};
use std::collections::BTreeMap;
use zf_context::context::ContextStrategy;
use zf_context::context_source;
use zf_core::types::DataType;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
use zf_storage::context_store;

fn nested_type(depth: usize) -> DataType {
    (0..depth).fold(DataType::Text, |item, _| DataType::Record {
        fields: BTreeMap::from([("child".into(), item)]),
    })
}

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"type":"custom","position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}

fn frozen_flow() -> Composition {
    let strategy =
        ContextStrategy::new_v2("deep-export", "Deep export").require("deep", nested_type(64));
    let source = context_source::generate(&strategy).unwrap();
    let config = json!({"modelNode":"model","fanIn":"any","contextProgram":{
        "strategy":strategy,"source":source,"hash":context_store::hash(source.as_bytes()),
        "types":{},"bindings":{"deep":{"kind":"state","field":"input"}}
    }});
    serde_json::from_value(json!({"formatVersion":3,"id":"deep-flow","name":"Deep flow","revision":0,
        "nodes":[node("start","start",json!({})),node("context","context",config),
            node("model","model",json!({"contextNode":"context","provider":"fixture","fixtureSteps":[{"text":"fixture"}]})),
            node("end","end",json!({}))],
        "edges":[{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"end"}]
    })).unwrap()
}

#[test]
fn frozen_context_depth_64_survives_flow_source_and_exact_cargo_export() {
    let flow = frozen_flow();
    let source = format!(
        "// Preserve the authored source exactly.\n{}",
        flow_source::render(
            &flow,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives
            )
        )
        .unwrap()
    );
    let restored = flow_source::parse(
        &source,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(&flow).unwrap()
    );
    let exported = zf_compiler::export::export_single(
        &zf_flows::flow_source::parse(
            &source,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
        &source,
        None,
        &zf_runtime::materialize::RuntimePrimitives,
        &zf_runtime::runtime_export::support(),
    )
    .unwrap();
    let exported_source = std::str::from_utf8(&exported.files["flows/instance-0/flow.rs"]).unwrap();
    assert_eq!(exported_source, source);
    assert_eq!(
        serde_json::to_value(
            flow_source::parse(
                exported_source,
                &zf_compiler::graph_compiler::GraphValidator::new(
                    &zf_runtime::materialize::RuntimePrimitives
                )
            )
            .unwrap()
        )
        .unwrap(),
        serde_json::to_value(flow).unwrap()
    );
}

#[test]
fn frozen_context_semantic_depth_65_remains_rejected() {
    let mut flow = frozen_flow();
    let program = &mut flow.nodes[1].data.config["contextProgram"];
    let source = program["source"].as_str().unwrap().replace(
        "DataType::Text",
        "DataType::Record { fields: BTreeMap::from([(\"child\".into(), DataType::Text)]) }",
    );
    program["hash"] = json!(context_store::hash(source.as_bytes()));
    program["source"] = json!(source);
    program["strategy"]["requirements"]["deep"] = serde_json::to_value(nested_type(65)).unwrap();
    let error = flow_source::render(
        &flow,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("64 levels"), "{error}");
}

#[test]
fn flow_source_keeps_a_separate_json_representation_limit() {
    let mut flow = frozen_flow();
    let mut value = json!(0);
    for _ in 0..255 {
        value = json!([value]);
    }
    flow.nodes[0].data.config = json!({"deep":value});
    let source = flow_source::render(
        &flow,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert!(
        flow_source::parse(
            &source,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives
            )
        )
        .is_ok()
    );
    flow.nodes[0].data.config["deep"] = json!([flow.nodes[0].data.config["deep"]]);
    let source = flow_source::render(
        &flow,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let error = flow_source::parse(
        &source,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("256 niveaux"), "{error}");
}

#[tokio::test]
async fn exact_cargo_bundle_executes_with_context_schema_depth_64() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let source = flow_source::render(
        &frozen_flow(),
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let exported = zf_compiler::export::export_single(
        &zf_flows::flow_source::parse(
            &source,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
        &source,
        None,
        &zf_runtime::materialize::RuntimePrimitives,
        &zf_runtime::runtime_export::support(),
    )
    .unwrap();
    let project = tempfile::tempdir().unwrap();
    for (relative, content) in &exported.files {
        let path = project.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let workspace = tempfile::tempdir().unwrap();
    let input = (0..64).fold(json!("deep fixture"), |item, _| json!({"child":item}));
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["run", "--quiet", "--offline", "--manifest-path"])
            .arg(project.path().join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(workspace.path())
            .arg("--home")
            .arg(workspace.path().join(".fixture-home"))
            .arg("--data")
            .arg(workspace.path().join("data"))
            .args(["--run-id", "deep-portable", "--input"])
            .arg(json!({"input":input}).to_string())
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .unwrap_or_else(|| "/tmp/zedflow-adk-target".into()),
            )
            .output(),
    )
    .await
    .expect("deep exported runtime timed out")
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = zf_context::context_json::from_slice(&output.stdout).unwrap();
    assert_eq!(state["status"], "completed", "{state}");
    let state = &state["state"];
    assert_eq!(state["output"], "fixture");
    assert_eq!(state["input"], input);
}
