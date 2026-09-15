use serde_json::json;
use zf_context::context_package::BridgeArtifactValidator;

#[test]
fn package_manifest_separates_identity_and_declared_files_from_rust_format() {
    use zf_flows::package::FlowPackageManifest;
    let value = json!({
        "formatVersion":1,"id":"working-system","name":"Working System",
        "entry":"flow.rs","files":["flow.rs","README.md","src/terms.rs","assets/example.json"],
        "dependencies":{"shared":{"path":"../shared-types"}}
    });
    let manifest = FlowPackageManifest::parse(&value.to_string()).unwrap();
    assert_eq!(manifest.id.as_str(), "working-system");
    assert_eq!(manifest.dependencies["shared"].path, "../shared-types");
    assert_eq!(serde_json::to_value(manifest).unwrap(), value);
    for path in [
        "../outside.rs",
        "/absolute.rs",
        "a/../outside.rs",
        "a\\outside.rs",
        "target/code.rs",
        "build.rs",
    ] {
        let mut invalid = value.clone();
        invalid["files"] = json!(["flow.rs", path]);
        assert!(
            FlowPackageManifest::parse(&invalid.to_string()).is_err(),
            "accepted {path}"
        );
    }
    let mut invalid = value.clone();
    invalid["files"] = json!(["README.md"]);
    assert!(FlowPackageManifest::parse(&invalid.to_string()).is_err());
    invalid = value.clone();
    invalid["formatVersion"] = json!(4);
    assert!(FlowPackageManifest::parse(&invalid.to_string()).is_err());
}

#[test]
fn flow_codec_preserves_all_versions_and_requires_the_callers_validation() {
    use zf_flows::{flow_format, schema::Composition};
    // This fixture validator checks the exact supported fixture, not a substitute
    // for the compiler's complete validation owned by P3.1.
    let fixture_validator = |doc: &Composition| -> anyhow::Result<()> {
        anyhow::ensure!(
            doc.nodes.len() == 2 && doc.edges.len() == 1,
            "fixture topology changed"
        );
        anyhow::ensure!(
            (1..=4).contains(&doc.format_version),
            "unsupported fixture version"
        );
        Ok(())
    };
    for version in 1..=4 {
        let doc: Composition = serde_json::from_value(json!({
            "formatVersion":version,"id":"fixture","name":"Été 🦀","revision":7,
            "nodes":[
                {"id":"s","position":{"x":12.5,"y":-9.25},"data":{"label":"Début","kind":"start","config":null}},
                {"id":"e","position":{"x":80,"y":0},"data":{"label":"Fin","kind":"end","config":{}}}
            ],"edges":[{"id":"edge","source":"s","target":"e","sourceHandle":null,"targetHandle":null,"label":"suite"}],
            "channels":[{"name":"custom","reducer":"overwrite","default":{"nested":["\u{0000}",42]}}]
        })).unwrap();
        let source = flow_format::render(&doc, &fixture_validator).unwrap();
        let read = flow_format::parse(&source, &fixture_validator).unwrap();
        assert_eq!(
            serde_json::to_value(read).unwrap(),
            serde_json::to_value(&doc).unwrap()
        );
        let reject =
            |_: &Composition| -> anyhow::Result<()> { anyhow::bail!("compiler rejected fixture") };
        assert!(
            flow_format::parse(&source, &reject)
                .unwrap_err()
                .to_string()
                .contains("compiler rejected fixture")
        );
        assert!(flow_format::render(&doc, &reject).is_err());
        assert!(
            flow_format::parse(&(source + "\nfn surprise() {}\n"), &fixture_validator).is_err()
        );
    }
}

#[test]
fn named_ports_distinguish_prepared_context_and_preserve_predicate_dependencies() {
    use zf_flows::{node_contracts, schema::Node};
    let node = |kind: &str, config| -> Node {
        serde_json::from_value(json!({"id":kind,"position":{"x":0,"y":0},"data":{"kind":kind,"label":kind,"config":config}})).unwrap()
    };
    let model = node_contracts::contract(&node("model", json!({})));
    assert_eq!(model.inputs[0].id, "context");
    assert_eq!(
        model.inputs[0].data_type,
        node_contracts::BoundaryType::PreparedContext
    );
    let condition = node_contracts::contract(&node(
        "condition",
        json!({"predicate":{"kind":"all","items":[
            {"kind":"compare","field":"/tool~1result/count","operator":"gte","value":1},
            {"kind":"compare","field":"ready","operator":"exists"}
        ]}}),
    ));
    assert_eq!(condition.consumes, ["ready", "tool/result"]);
    assert_eq!(
        condition
            .outputs
            .iter()
            .map(|p| p.label.as_str())
            .collect::<Vec<_>>(),
        ["Oui", "Non"]
    );
    assert!(
        node_contracts::parse_predicate(
            &json!({"kind":"compare","field":"count","operator":"gte","value":"1"})
        )
        .is_err()
    );
}

#[test]
fn public_entry_validates_its_type_and_maps_only_the_declared_input_channel() {
    let mut document: zf_flows::schema::Composition = serde_json::from_value(json!({
        "formatVersion":4,"id":"docs","name":"Docs","nodes":[
            {"id":"start","position":{"x":0,"y":0},"data":{"label":"Start","kind":"start","config":{
                "exports":{
                    "contract":{"entries":{"main":{"input":{"kind":"text"}}}},
                    "entries":{"main":{"node":"context","inputField":"request"}}
                }
            }}},
            {"id":"context","position":{"x":16,"y":0},"data":{"label":"Context","kind":"context","config":{}}}
        ],"edges":[],"channels":[{"name":"request"}]
    })).unwrap();
    let exports = zf_flows::flow_contract::validate(&document)
        .unwrap()
        .unwrap();
    let state =
        zf_flows::flow_contract::entry_input(&exports, "main", json!("Inspect docs")).unwrap();
    assert_eq!(
        serde_json::to_value(state).unwrap(),
        json!({"request":"Inspect docs"})
    );
    assert!(zf_flows::flow_contract::entry_input(&exports, "main", json!(42)).is_err());
    document.channels.clear();
    assert!(
        zf_flows::flow_contract::validate(&document)
            .unwrap_err()
            .to_string()
            .contains("undeclared channel request")
    );
}

#[test]
fn context_packages_obtain_dependencies_from_the_real_bridge_parser() {
    let source = r#"// @zedflow-bridge 1
use zf_flows::composition::*;
pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new().require("base").import("docs", "working-system")
}
"#;
    let dependencies = zf_flows::bridge_source::PackageBridgeValidator
        .validate(source)
        .unwrap();
    assert_eq!(dependencies.requires, ["base"]);
    assert_eq!(dependencies.flows, ["working-system"]);
    assert!(
        zf_flows::bridge_source::PackageBridgeValidator
            .validate(&source.replace(
                ".import(\"docs\", \"working-system\")",
                ".invented(\"docs\")"
            ))
            .is_err()
    );
}

#[test]
fn historical_bridge_keeps_explicit_routes_and_generates_standalone_imports() {
    let legacy = r#"// @zedflow-bridge 1
use zedflow_daemon::harness::composition::*;

pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        .require("base")
        .import("docs", "working-system")
        .connect("documentation", Connection::new(Endpoint::new("root", "work"), Endpoint::new("docs", "main"), RouteMode::CallAwait, InvocationKind::Node))
}
"#;
    let bridge = zf_flows::bridge_source::parse(legacy).unwrap();
    let encoded = serde_json::to_value(&bridge).unwrap();
    assert_eq!(encoded["requires"], json!(["base"]));
    assert_eq!(encoded["imports"]["docs"]["flow"], "working-system");
    assert_eq!(encoded["connections"]["documentation"]["mode"], "callAwait");
    assert_eq!(
        encoded["connections"]["documentation"]["invocation"],
        "node"
    );
    let generated = zf_flows::bridge_source::generate(&bridge).unwrap();
    assert!(generated.contains("use zf_flows::composition::*;"));
    assert!(!generated.contains("zedflow_daemon"));
    let reparsed = zf_flows::bridge_source::parse(&generated).unwrap();
    assert_eq!(serde_json::to_value(reparsed).unwrap(), encoded);
    assert!(
        zf_flows::bridge_source::parse(&legacy.replace(
            "BridgeDefinition::new()",
            "{ std::process::exit(1); BridgeDefinition::new() }"
        ))
        .is_err()
    );
}

// Existing historical catalogue fixture retained at the codec boundary.
mod fixtures {
    use serde_json::{Value, json};
    use zf_flows::schema::Composition;
    fn node(id: &str, kind: &str, config: Value) -> Value {
        json!({"id":id,"type":"flow","position":{"x":12.5,"y":-9.25},"data":{"label":format!("Nœud {id}"),"kind":kind,"config":config}})
    }

    fn flow(nodes: Vec<Value>, edges: &[(&str, &str)]) -> Composition {
        serde_json::from_value(json!({"id":"source-test","name":"Source UTF-8 é 🦀","revision":7,"nodes":nodes,
            "edges":edges.iter().enumerate().map(|(i,(source,target))|json!({"id":format!("edge-{i}"),"source":source,"target":target,"label":format!("Lien {i}")})).collect::<Vec<_>>() })).unwrap()
    }

    fn basic() -> Composition {
        flow(
            vec![
                node("s", "start", Value::Null),
                node("set", "set", json!({"field":"output","value":"before"})),
                node("e", "end", json!({})),
            ],
            &[("s", "set"), ("set", "e")],
        )
    }

    pub fn catalog() -> Composition {
        let child = flow(
            vec![
                node("s", "start", json!({})),
                node("nested", "subgraph", json!({"composition":basic()})),
                node("e", "end", json!({})),
            ],
            &[("s", "nested"), ("nested", "e")],
        );
        let mut doc = flow(
            vec![
                node("s", "start", json!({"viewOnly":"retained"})),
                node("context", "context", json!({})),
                node("set", "set", json!({"field":"count","value":-3})),
                node(
                    "input",
                    "input",
                    json!({"prompt":"Question ?","field":"input"}),
                ),
                node(
                    "model",
                    "agent",
                    json!({"modelBinding":"runtime","tools":["read","write","edit","exec"],"temperature":0.2,"retry":{"maxAttempts":2,"initialDelayMs":1,"maxDelayMs":2}}),
                ),
                node("tools", "tool", json!({"tool":"execute_next_call"})),
                node("steering", "steering", json!({})),
                node("inbox", "inbox", json!({})),
                node("route", "condition", json!({"field":"count","equals":-3})),
                node(
                    "child",
                    "subgraph",
                    json!({"composition":child,"fanIn":"any"}),
                ),
                node(
                    "output",
                    "output",
                    json!({"text":"{{output}}","ui":{"renderer":"json","title":"\u{0000}\u{0008}\u{000c}\n\t\\\"🦀"},"extensions":{"a":[null,true,false,18446744073709551615_u64,-9223372036854775808_i64,1e100]}}),
                ),
                node("e", "end", json!({})),
                node("no", "end", Value::Null),
            ],
            &[
                ("s", "context"),
                ("context", "set"),
                ("set", "input"),
                ("input", "model"),
                ("model", "tools"),
                ("tools", "steering"),
                ("steering", "inbox"),
                ("inbox", "route"),
                ("route", "child"),
                ("route", "no"),
                ("child", "output"),
                ("output", "e"),
            ],
        );
        doc.edges[8].source_handle = Some("true".into());
        doc.edges[9].source_handle = Some("false".into());
        doc.edges[0].source_handle = Some("display-only".into());
        doc.nodes[0].node_type = "special presentation".into();
        doc.settings = serde_json::from_value(json!({"maxConcurrency":2,"strictChannels":true,"recursionLimit":111,"timeoutMs":5000,"idleTimeoutMs":3000,"retry":{"maxAttempts":2,"initialDelayMs":1,"maxDelayMs":4,"backoffFactor":1.5,"jitter":0.25,"retryOn":"timeout"}})).unwrap();
        doc.channels = serde_json::from_value(json!([{"name":"count","reducer":"sum","default":10},{"name":"items","reducer":"append","default":["old"]},{"name":"custom","reducer":"overwrite","default":{"nested":true}}])).unwrap();
        doc
    }
}

#[test]
fn nested_historical_source_keeps_literals_channels_and_conversion_as_a_copy() {
    use zf_flows::{flow_format, schema::Composition};
    let fixture_contracts = |doc: &Composition| -> anyhow::Result<()> {
        zf_flows::schema::channel_names(&doc.channels)?;
        zf_flows::flow_contract::validate(doc)?;
        Ok(())
    };
    let document = fixtures::catalog();
    let original = serde_json::to_value(&document).unwrap();
    let source = flow_format::render(&document, &fixture_contracts).unwrap();
    // Every import in this generated fixture is a prelude; the fixture contains
    // no matching string literal. Historical sources use the old root modules.
    let legacy = source.replace(
        "use zf_runtime::{models, operations, runtime, subgraphs};",
        "use crate::{models, operations, runtime, subgraphs};",
    );
    let loaded = flow_format::parse(&legacy, &fixture_contracts).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), original);
    assert!(source.contains("mod subgraph_9"));
    assert!(!source.contains("\"composition\":"));
    assert_eq!(
        zf_flows::schema::answer_paths(&loaded).unwrap(),
        ["input", "inbox"]
    );
    let channels = zf_flows::schema::runtime_channels(&loaded).unwrap();
    assert!(
        channels
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == &json!({"name":"answer:inbox","reducer":"overwrite"}))
    );
    for changed in [
        source.replace("\"value\": -3", "\"value\": std::env::var(\"SECRET\")"),
        source.replace("state.get(&field_8)", "state.get(\"wrong\")"),
        format!("{source}\nmod external;"),
    ] {
        assert_ne!(
            changed, source,
            "fixture mutation must actually change the source"
        );
        assert!(flow_format::parse(&changed, &fixture_contracts).is_err());
    }
    let converted = flow_format::convert_v1(&loaded, &fixture_contracts).unwrap();
    assert_ne!(converted.id, loaded.id);
    assert_eq!(converted.format_version, 2);
    assert_eq!(converted.revision, 0);
    assert_eq!(serde_json::to_value(&loaded).unwrap(), original);
    let converted_source = flow_format::render(&converted, &fixture_contracts).unwrap();
    assert_eq!(
        serde_json::to_value(flow_format::parse(&converted_source, &fixture_contracts).unwrap())
            .unwrap(),
        serde_json::to_value(converted).unwrap()
    );
}
