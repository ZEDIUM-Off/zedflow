use serde_json::{Value, json};
use std::collections::BTreeMap;
use zf_compiler::programs;
use zf_context::context::ContextBlock;
use zf_context::context::ContextExpr;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_core::types::DataType;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
use zf_storage::context_store::ContextStore;
use zf_storage::context_store::TypeStore;

async fn fixture(workspace: &std::path::Path) -> Composition {
    let types = BTreeMap::from([(
        "Report".into(),
        DataType::Record {
            fields: BTreeMap::from([("title".into(), DataType::Text)]),
        },
    )]);
    let types = TypeStore::new(workspace.into())
        .save("domain", &types, None)
        .await
        .unwrap();
    let strategy = ContextStrategy::new("read-title", "Read title")
        .require(
            "report",
            DataType::Named {
                name: "Report".into(),
            },
        )
        .with_program(vec![ContextBlock::emit(
            "title",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::field(ContextExpr::resource("report"), "title"),
        )]);
    let strategy = ContextStore::new(workspace.into())
        .save(&strategy, None)
        .await
        .unwrap();
    let mut doc:Composition = serde_json::from_value(json!({"formatVersion":3,"id":"portable-reader","name":"Portable reader","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"agent","position":{"x":0,"y":0},"data":{"kind":"agent","label":"Reader","config":{"provider":"fixture","fixtureSteps":[{"echoRequest":true}],"contextStrategy":{"key":"read-title","hash":strategy.hash},"contextTypesRef":{"key":"domain","hash":types.hash},"contextBindings":{"report":{"kind":"reader","reader":"file.json","input":{"kind":"state","field":"source"}}}}}},
        {"id":"end","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"start","target":"agent"},{"id":"b","source":"agent","target":"end"}]})).unwrap();
    async {
        let sources = zf_execution::sources::program_sources(&doc, workspace, &[]).await?;
        programs::freeze(&mut doc, &sources)
    }
    .await
    .unwrap();
    doc
}

#[tokio::test]
async fn export_bundles_standard_reader_and_exact_frozen_types_and_rejects_missing_native_extensions()
 {
    let workspace = tempfile::tempdir().unwrap();
    let doc = fixture(workspace.path()).await;
    let source = flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let generated = zf_compiler::export::export_single(
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
    let files = &generated.files;
    assert!(files.contains_key("crates/zf-runtime/src/resources.rs"));
    assert_eq!(files["flows/instance-0/flow.rs"], source.as_bytes());
    let restored = flow_source::parse(
        &source,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert_eq!(
        restored.nodes[1].data.config["contextProgram"]["typeSources"],
        doc.nodes[1].data.config["contextProgram"]["typeSources"]
    );
    let mut missing = doc.clone();
    missing.nodes[1].data.config["contextProgram"]["bindings"]["report"]["input"] =
        json!({"kind":"literal","value":{"path":"missing-resource.json"}});
    let diagnostics = zf_runtime::resources::dependency_diagnostics(&missing, workspace.path());
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("only if"));
    assert!(
        zf_runtime::resources::dependency_diagnostics(&doc, workspace.path()).is_empty(),
        "State inputs are decided at the actual passage"
    );
    let mut unknown = doc.clone();
    unknown.nodes[1].data.config["contextProgram"]["bindings"]["report"]["reader"] =
        json!("application.private-native-reader");
    assert!(
        zf_compiler::export::export_single(
            &unknown,
            &zf_flows::flow_source::render(
                &unknown,
                &zf_compiler::graph_compiler::GraphValidator::new(
                    &zf_runtime::materialize::RuntimePrimitives
                )
            )
            .unwrap(),
            None,
            &zf_runtime::materialize::RuntimePrimitives,
            &zf_runtime::runtime_export::support()
        )
        .unwrap_err()
        .to_string()
        .contains("native reader implementation absent from Cargo export: application.private-native-reader")
    );

    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    for (relative, content) in files {
        let path = directory.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    // Execute elsewhere after every authoring source has been removed. Native
    // reader data is explicit; no workspace/global source catalog is consulted.
    drop(workspace);
    let target = tempfile::tempdir().unwrap();
    std::fs::write(
        target.path().join("report.json"),
        r#"{"title":"Portable source exact"}"#,
    )
    .unwrap();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["run", "--offline", "--quiet", "--manifest-path"])
            .arg(directory.path().join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(target.path())
            .arg("--home")
            .arg(target.path().join(".fixture-home"))
            .arg("--data")
            .arg(target.path().join("data"))
            .args([
                "--run-id",
                "portable-reader",
                "--input",
                r#"{"source":{"path":"report.json"}}"#,
            ])
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .unwrap_or_else(|| "/tmp/zedflow-adk-target".into()),
            )
            .output(),
    )
    .await
    .expect("Portable reader timed out")
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(state["status"], "completed");
    let request: Value = serde_json::from_str(state["state"]["output"].as_str().unwrap()).unwrap();
    assert_eq!(
        request["contents"][0]["parts"][0]["text"],
        "Portable source exact"
    );
}
