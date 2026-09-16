use serde_json::json;
use std::collections::BTreeMap;
use zf_compiler::{
    export::{RuntimeSupport, export_single},
    graph_compiler::{GraphValidator, PrimitiveContracts},
};
use zf_flows::{flow_format, schema::Composition};

struct Primitives;
impl PrimitiveContracts for Primitives {
    fn validate_model(&self, _: &serde_json::Value) -> anyhow::Result<()> {
        anyhow::bail!("fixture has no models")
    }
    fn has_tool(&self, _: &str) -> bool {
        false
    }
}
fn fixture() -> Composition {
    serde_json::from_value(
        json!({"formatVersion":2,"id":"single","name":"Single","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"end","position":{"x":0,"y":1},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"edge","source":"start","target":"end"}]}),
    )
    .unwrap()
}
fn support() -> RuntimeSupport {
    let internal = [
        "zf-core",
        "zf-context",
        "zf-flows",
        "zf-compiler",
        "zf-storage",
        "zf-runtime",
        "zf-execution",
    ];
    let mut files =
        BTreeMap::from([("Cargo.toml".into(), b"[workspace]\nmembers = []\n".to_vec())]);
    let mut lock = String::from("version = 4\n\n");
    for name in internal
        .iter()
        .chain(["adk-graph", "anyhow", "serde_json", "tokio"].iter())
    {
        lock.push_str(&format!(
            "[[package]]\nname = {name:?}\nversion = \"1.0.0\"\n\n"
        ));
    }
    for name in internal {
        files.insert(format!("crates/{name}/Cargo.toml"), Vec::new());
        files.insert(format!("crates/{name}/src/lib.rs"), Vec::new());
    }
    files.insert("Cargo.lock".into(), lock.into_bytes());
    RuntimeSupport { files }
}

#[test]
fn single_flow_preserves_rust_without_inventing_ports() {
    let doc = fixture();
    let source = format!(
        "{}\n// retain exact bytes\r\n",
        flow_format::render(&doc, &GraphValidator::new(&Primitives)).unwrap()
    );
    let export = export_single(&doc, &source, None, &Primitives, &support()).unwrap();
    assert_eq!(export.files["flows/instance-0/flow.rs"], source.as_bytes());
    let definition: serde_json::Value =
        serde_json::from_slice(&export.files["definition.json"]).unwrap();
    assert_eq!(definition["source"], source);
    assert!(definition["composition"]["nodes"][0]["data"]["config"]["exports"].is_null());
    assert!(
        std::str::from_utf8(&export.files["runner/src/main.rs"])
            .unwrap()
            .contains("run_single")
    );
}

#[test]
fn single_flow_rejects_mismatched_projection_and_unsafe_support_paths() {
    let doc = fixture();
    let source = flow_format::render(&doc, &GraphValidator::new(&Primitives)).unwrap();
    let mut changed = doc.clone();
    changed.name = "Different".into();
    assert!(
        export_single(&changed, &source, None, &Primitives, &support())
            .unwrap_err()
            .to_string()
            .contains("disagree")
    );
    let mut invalid = support();
    invalid.files.insert("../escape".into(), vec![]);
    assert!(
        export_single(&doc, &source, None, &Primitives, &invalid)
            .unwrap_err()
            .to_string()
            .contains("path")
    );
}

#[test]
fn supported_rust_versions_keep_legacy_imports_and_source_bytes() {
    for version in [1, 2, 3, 4] {
        let mut doc = fixture();
        doc.format_version = version;
        let source = flow_format::render(&doc, &GraphValidator::new(&Primitives)).unwrap();
        for source in [
            source.clone(),
            source.replace(
                "use zf_runtime::{models, operations, runtime, subgraphs};",
                "use crate::{models, operations, runtime, subgraphs};",
            ),
        ] {
            let bundle = export_single(&doc, &source, None, &Primitives, &support()).unwrap();
            assert_eq!(bundle.files["flows/instance-0/flow.rs"], source.as_bytes());
        }
    }
}

#[test]
fn frozen_runtime_roundtrip_is_revalidated_without_catalogue_resolution() {
    use zf_compiler::{
        compiler::{CompileRequest, compile, compile_prepared},
        prepared::CompilationSnapshot,
        programs::SourceSnapshot,
    };
    let mut doc = fixture();
    doc.nodes[0].data.config = json!({"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":false}});
    let source = flow_format::render(&doc, &GraphValidator::new(&Primitives)).unwrap();
    let first = compile(
        &CompilationSnapshot {
            flows: BTreeMap::from([("single".into(), SourceSnapshot::capture(source))]),
            ..Default::default()
        },
        &CompileRequest::new(zf_flows::composition::ResolveRequest {
            flow: "single".into(),
            entry: "main".into(),
            bridges: vec![],
        }),
        &Primitives,
    )
    .unwrap();
    let restored = serde_json::from_slice(&serde_json::to_vec(first.prepared()).unwrap()).unwrap();
    let again = compile_prepared(restored, &Primitives).unwrap();
    assert_eq!(first.revision(), again.revision());
    let mut corrupt = again.prepared().clone();
    corrupt
        .flows
        .get_mut("root")
        .unwrap()
        .source
        .push_str("\n// changed");
    assert!(
        compile_prepared(corrupt, &Primitives)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == "prepared_validation")
    );
}

#[test]
fn historical_single_export_keeps_authored_package_and_executed_context_selection() {
    use zf_compiler::{
        export::export_single_with_context,
        prepared_model::ContextSelection,
        programs::{ProgramSources, SourceSnapshot},
    };
    use zf_context::{context::ContextStrategy, context_source};
    use zf_flows::package::PackageSnapshot;
    struct ModelPrimitives;
    impl PrimitiveContracts for ModelPrimitives {
        fn validate_model(&self, config: &serde_json::Value) -> anyhow::Result<()> {
            anyhow::ensure!(config["provider"] == "fixture", "fixture provider required");
            Ok(())
        }
        fn has_tool(&self, _: &str) -> bool {
            false
        }
    }
    let primitives = ModelPrimitives;
    let mut doc = fixture();
    doc.format_version = 3;
    doc.nodes.splice(1..1, serde_json::from_value::<Vec<zf_flows::schema::Node>>(json!([
        {"id":"context","position":{"x":0,"y":1},"data":{"kind":"context","label":"Context","config":{"modelNode":"model","contextStrategy":"original"}}},
        {"id":"model","position":{"x":0,"y":2},"data":{"kind":"model","label":"Model","config":{"provider":"fixture","contextNode":"context"}}}
    ])).unwrap());
    doc.edges = serde_json::from_value(json!([{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"end"}])).unwrap();
    let authored = flow_format::render(&doc, &GraphValidator::new(&primitives)).unwrap();
    assert!(
        export_single(&doc, &authored, None, &primitives, &support())
            .unwrap_err()
            .to_string()
            .contains("unresolved context")
    );
    let mut nested = fixture();
    nested.format_version = 3;
    nested.nodes.insert(1, serde_json::from_value(json!({"id":"nested","position":{"x":0,"y":1},"data":{"kind":"subgraph","label":"Nested","config":{"composition":doc}}})).unwrap());
    nested.edges = serde_json::from_value(json!([{"id":"a","source":"start","target":"nested"},{"id":"b","source":"nested","target":"end"}])).unwrap();
    let nested_source = flow_format::render(&nested, &GraphValidator::new(&primitives)).unwrap();
    assert!(
        export_single(&nested, &nested_source, None, &primitives, &support())
            .unwrap_err()
            .to_string()
            .contains("unresolved context")
    );
    let mut agent = fixture();
    agent.nodes.insert(1, serde_json::from_value(json!({"id":"agent","position":{"x":0,"y":1},"data":{"kind":"agent","label":"Agent","config":{"provider":"fixture","contextStrategy":"original"}}})).unwrap());
    agent.edges = serde_json::from_value(json!([{"id":"a","source":"start","target":"agent"},{"id":"b","source":"agent","target":"end"}])).unwrap();
    let agent_source = flow_format::render(&agent, &GraphValidator::new(&primitives)).unwrap();
    assert!(
        export_single(&agent, &agent_source, None, &primitives, &support())
            .unwrap_err()
            .to_string()
            .contains("unresolved context")
    );
    let package = PackageSnapshot::capture(json!({"formatVersion":1,"id":"single","name":"Single","entry":"flow.rs","files":["flow.rs"],"dependencies":{}}).to_string(),
        BTreeMap::from([("flow.rs".into(),authored.as_bytes().to_vec())]),BTreeMap::new()).unwrap();
    let alternate = SourceSnapshot::capture(
        context_source::generate(&ContextStrategy::new("alternate", "Alternate")).unwrap(),
    );
    let selections = BTreeMap::from([(
        "model".into(),
        ContextSelection {
            key: "alternate".into(),
            hash: alternate.hash.clone(),
        },
    )]);
    zf_compiler::prepared::apply_context_selections(&mut doc, &selections, None).unwrap();
    zf_compiler::programs::freeze(
        &mut doc,
        &ProgramSources {
            strategies: BTreeMap::from([("alternate".into(), alternate)]),
            ..Default::default()
        },
    )
    .unwrap();
    let executed = flow_format::render(&doc, &GraphValidator::new(&primitives)).unwrap();
    assert_ne!(authored, executed);
    assert!(export_single(&doc, &executed, Some(&package), &primitives, &support()).is_err());
    let exported = export_single_with_context(
        &doc,
        &executed,
        Some(&package),
        &selections,
        &primitives,
        &support(),
    )
    .unwrap();
    assert_eq!(
        exported.files["flows/instance-0/flow.rs"],
        executed.as_bytes()
    );
    assert_eq!(
        exported.files[&format!("packages/{}/flow.rs", package.root)],
        authored.as_bytes()
    );
    let definition: serde_json::Value =
        serde_json::from_slice(&exported.files["definition.json"]).unwrap();
    assert_eq!(
        definition["contextSelections"],
        serde_json::to_value(selections).unwrap()
    );
}
