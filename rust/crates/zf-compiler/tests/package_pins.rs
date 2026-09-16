use serde_json::json;
use std::collections::BTreeMap;
use zf_compiler::{
    graph_compiler::GraphValidator,
    prepared::{CompilationSnapshot, PreparedRuntime, prepare},
    programs::SourceSnapshot,
};
use zf_flows::{composition::ResolveRequest, flow_format, package::PackageSnapshot};

struct FixturePrimitives;
impl zf_compiler::graph_compiler::PrimitiveContracts for FixturePrimitives {
    fn validate_model(&self, config: &serde_json::Value) -> anyhow::Result<()> {
        anyhow::ensure!(config["provider"] == "fixture", "Fixture provider required");
        anyhow::ensure!(
            config["temperature"]
                .as_f64()
                .is_none_or(|v| (0.0..=2.0).contains(&v)),
            "Invalid temperature"
        );
        Ok(())
    }
    fn has_tool(&self, name: &str) -> bool {
        name == "inspect_json"
    }
}
fn flow_node(id: &str, kind: &str, config: serde_json::Value) -> serde_json::Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn exposed_flow(id: &str, root: bool) -> zf_flows::schema::Composition {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":false});
    if root {
        exports["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["node"]}});
        exports["branches"] = json!({"work":"action"});
    }
    let action = if root {
        flow_node(
            "action",
            "route",
            json!({"branch":"work","inputField":"input","field":"output"}),
        )
    } else {
        flow_node(
            "action",
            "set",
            json!({"field":"output","value":"child result"}),
        )
    };
    serde_json::from_value(json!({"id":id,"name":id,"formatVersion":2,"nodes":[flow_node("start","start",json!({"exports":exports})), action,flow_node("end","end",json!({}))],"edges":[{"id":"a","source":"start","target":"action"},{"id":"b","source":"action","target":"end"}]})).unwrap()
}

fn package(
    id: &str,
    source: &str,
    asset: &str,
    dependency: Option<PackageSnapshot>,
) -> PackageSnapshot {
    let dependencies = dependency
        .map(|p| BTreeMap::from([("local".into(), p)]))
        .unwrap_or_default();
    let manifest = json!({"formatVersion":1,"id":id,"name":"shared human name","entry":"flow.rs","files":["flow.rs","README.md"],"dependencies":if dependencies.is_empty() { json!({}) } else { json!({"local":{"path":"../dependency"}}) }});
    PackageSnapshot::capture(
        manifest.to_string(),
        BTreeMap::from([
            ("flow.rs".into(), source.as_bytes().to_vec()),
            ("README.md".into(), asset.as_bytes().to_vec()),
        ]),
        dependencies,
    )
    .unwrap()
}
fn snapshot() -> CompilationSnapshot {
    let source = flow_format::render(
        &exposed_flow("worker", false),
        &GraphValidator::new(&FixturePrimitives),
    )
    .unwrap();
    CompilationSnapshot {
        flows: BTreeMap::from([("worker".into(), SourceSnapshot::capture(source.clone()))]),
        packages: BTreeMap::from([("worker".into(), package("worker", &source, "first", None))]),
        ..Default::default()
    }
}
fn compile(snapshot: &CompilationSnapshot) -> anyhow::Result<PreparedRuntime> {
    prepare(
        snapshot,
        &ResolveRequest {
            flow: "worker".into(),
            entry: "main".into(),
            bridges: vec![],
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        &FixturePrimitives,
    )
}
#[test]
fn package_capture_is_stable_when_secondary_and_dependency_sources_change() {
    let mut input = snapshot();
    let source = input.flows["worker"].source.clone();
    let first_dep = package("dep", "dependency entry", "first dependency", None);
    input.packages.insert(
        "worker".into(),
        package("worker", &source, "first", Some(first_dep)),
    );
    let frozen = compile(&input).unwrap();
    let serialized = serde_json::to_string(&frozen).unwrap();
    let first_revision = input.packages["worker"].root.clone();
    let next_dep = package("dep", "dependency entry", "next dependency", None);
    input.packages.insert(
        "worker".into(),
        package("worker", &source, "first", Some(next_dep.clone())),
    );
    assert_ne!(first_revision, input.packages["worker"].root);
    let dependency_revision = input.packages["worker"].root.clone();
    input.packages.insert(
        "worker".into(),
        package("worker", &source, "next", Some(next_dep)),
    );
    assert_ne!(dependency_revision, input.packages["worker"].root);
    assert_eq!(serialized, serde_json::to_string(&frozen).unwrap());
    frozen.validate(&FixturePrimitives).unwrap();
    assert_eq!(
        frozen.definitions.flow_hashes["worker"],
        input.flows["worker"].hash
    );
    assert_eq!(
        frozen.summary()["packageRevisions"]["worker"],
        first_revision
    );
    assert!(frozen.summary()["packageRevisions"]["worker"].is_string());
    let stale = BTreeMap::from([("worker".into(), first_revision)]);
    let request = ResolveRequest {
        flow: "worker".into(),
        entry: "main".into(),
        bridges: vec![],
    };
    assert!(
        prepare(
            &input,
            &request,
            &stale,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
    let current = BTreeMap::from([("worker".into(), input.packages["worker"].root.clone())]);
    prepare(
        &input,
        &request,
        &current,
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap();
}
#[test]
fn package_compilation_rejects_identity_source_hash_and_closure_mismatch() {
    let original = snapshot();
    let mut input = original.clone();
    input.packages.insert(
        "worker".into(),
        package("impostor", &input.flows["worker"].source, "first", None),
    );
    assert!(
        compile(&input)
            .unwrap_err()
            .to_string()
            .contains("identity")
    );
    input = original.clone();
    input.flows.get_mut("worker").unwrap().source.push('\n');
    input.flows.get_mut("worker").unwrap().hash =
        zf_compiler::programs::hash(input.flows["worker"].source.as_bytes());
    assert!(
        compile(&input)
            .unwrap_err()
            .to_string()
            .contains("disagree")
    );
    input = original.clone();
    input.flows.get_mut("worker").unwrap().hash = "0".repeat(64);
    assert!(
        compile(&input)
            .unwrap_err()
            .to_string()
            .contains("hash mismatch")
    );
    input = original.clone();
    let package = input.packages.get_mut("worker").unwrap();
    package
        .packages
        .get_mut(&package.root)
        .unwrap()
        .files
        .insert("README.md".into(), b"corrupted".to_vec());
    assert!(compile(&input).is_err());
    input = original.clone();
    input
        .packages
        .insert("orphan".into(), original.packages["worker"].clone());
    assert!(compile(&input).unwrap_err().to_string().contains("Orphan"));
}

#[test]
fn secondary_file_references_are_checked_at_preparation_and_restoration() {
    let mut input = snapshot();
    let capture = |secondary: &str| {
        PackageSnapshot::capture(
            json!({"formatVersion":1,"id":"worker","name":"Worker","entry":"flow.rs","files":["flow.rs","secondary.rs"]}).to_string(),
            BTreeMap::from([
                ("flow.rs".into(), input.flows["worker"].source.as_bytes().to_vec()),
                ("secondary.rs".into(), secondary.as_bytes().to_vec()),
            ]), BTreeMap::new(),
        ).unwrap()
    };
    let safe = capture("pub const DATA: &str = \"captured\";");
    let outside = capture("pub const DATA: &str = include_str!(\"/tmp/outside.txt\");");
    input.packages.insert("worker".into(), safe);
    let mut prepared = compile(&input).unwrap();
    input.packages.insert("worker".into(), outside.clone());
    let error = compile(&input).unwrap_err();
    assert!(format!("{error:#}").contains("secondary.rs"));
    prepared
        .definitions
        .flow_packages
        .insert("worker".into(), outside);
    let error = prepared.validate(&FixturePrimitives).unwrap_err();
    assert!(format!("{error:#}").contains("secondary.rs"));
}
#[test]
fn runtime_rejects_rehashed_executable_substitution_and_orphan_pins() {
    let input = snapshot();
    let original = compile(&input).unwrap();
    let mut frozen = original.clone();
    let flow = frozen.flows.get_mut("root").unwrap();
    flow.composition.nodes[1].data.config["value"] = json!("malicious replacement");
    flow.source =
        flow_format::render(&flow.composition, &GraphValidator::new(&FixturePrimitives)).unwrap();
    flow.hash = zf_compiler::programs::hash(flow.source.as_bytes());
    assert!(
        frozen
            .validate(&FixturePrimitives)
            .unwrap_err()
            .to_string()
            .contains("beyond context linking")
    );
    frozen = original.clone();
    frozen
        .definitions
        .flow_hashes
        .insert("worker".into(), "0".repeat(64));
    assert!(frozen.validate(&FixturePrimitives).is_err());
    frozen = original.clone();
    frozen
        .definitions
        .flow_packages
        .insert("orphan".into(), input.packages["worker"].clone());
    assert!(
        frozen
            .validate(&FixturePrimitives)
            .unwrap_err()
            .to_string()
            .contains("Orphan")
    );
    frozen = original.clone();
    frozen.flows.get_mut("root").unwrap().key = "unrelated".into();
    assert!(frozen.validate(&FixturePrimitives).is_err());
    frozen = original;
    frozen.definitions.flow_packages.insert(
        "worker".into(),
        package("wrong-id", &input.flows["worker"].source, "first", None),
    );
    assert!(
        frozen
            .validate(&FixturePrimitives)
            .unwrap_err()
            .to_string()
            .contains("identity")
    );
}
#[test]
fn historical_serializations_without_package_fields_remain_valid() {
    let mut input = snapshot();
    input.packages.clear();
    let historical = serde_json::to_value(&input).unwrap();
    assert!(historical.get("packages").is_none());
    let decoded: CompilationSnapshot = serde_json::from_value(historical).unwrap();
    let prepared = compile(&decoded).unwrap();
    let mut historical = serde_json::to_value(prepared).unwrap();
    assert!(historical["definitions"].get("flowPackages").is_none());
    let decoded: PreparedRuntime = serde_json::from_value(historical.clone()).unwrap();
    decoded.validate(&FixturePrimitives).unwrap();
    historical.as_object_mut().unwrap().remove("definitions");
    let decoded: PreparedRuntime = serde_json::from_value(historical).unwrap();
    decoded.validate(&FixturePrimitives).unwrap();
}
#[test]
fn multiple_instances_share_one_package_pin() {
    use zf_flows::{bridge_source, composition::*};
    let mut input = snapshot();
    let pilot = flow_format::render(
        &exposed_flow("pilot", true),
        &GraphValidator::new(&FixturePrimitives),
    )
    .unwrap();
    input
        .flows
        .insert("pilot".into(), SourceSnapshot::capture(pilot));
    for id in ["a", "b"] {
        let bridge = BridgeDefinition::new().import("worker", "worker").connect(
            "work",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("worker", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        );
        input.bridges.insert(
            id.into(),
            SourceSnapshot::capture(bridge_source::generate(&bridge).unwrap()),
        );
    }
    let request = ResolveRequest {
        flow: "pilot".into(),
        entry: "main".into(),
        bridges: vec!["a".into(), "b".into()],
    };
    let prepared = prepare(
        &input,
        &request,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap();
    assert_eq!(prepared.flows.len(), 3);
    assert_eq!(prepared.definitions.flow_packages.len(), 1);
    let encoded = serde_json::to_value(&prepared).unwrap();
    assert_eq!(
        encoded["definitions"]["flowPackages"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert!(encoded["flows"]["a/worker"].get("package").is_none());
    prepared.validate(&FixturePrimitives).unwrap();
}

#[test]
fn selected_flows_cannot_pin_different_revisions_of_one_dependency_identity() {
    use zf_flows::{bridge_source, composition::*};
    let mut input = snapshot();
    let pilot = flow_format::render(
        &exposed_flow("pilot", true),
        &GraphValidator::new(&FixturePrimitives),
    )
    .unwrap();
    let dependency = package("dependency", "// shared dependency", "one", None);
    input.packages.insert(
        "pilot".into(),
        package("pilot", &pilot, "pilot", Some(dependency.clone())),
    );
    input.packages.insert(
        "worker".into(),
        package(
            "worker",
            &input.flows["worker"].source,
            "worker",
            Some(dependency),
        ),
    );
    input
        .flows
        .insert("pilot".into(), SourceSnapshot::capture(pilot));
    let bridge = BridgeDefinition::new().import("worker", "worker").connect(
        "work",
        Connection::new(
            Endpoint::new("root", "work"),
            Endpoint::new("worker", "main"),
            RouteMode::CallAwait,
            InvocationKind::Node,
        ),
    );
    input.bridges.insert(
        "bridge".into(),
        SourceSnapshot::capture(bridge_source::generate(&bridge).unwrap()),
    );
    let request = ResolveRequest {
        flow: "pilot".into(),
        entry: "main".into(),
        bridges: vec!["bridge".into()],
    };
    let prepared = prepare(
        &input,
        &request,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap();
    prepared.validate(&FixturePrimitives).unwrap();
    let changed = package(
        "worker",
        &input.flows["worker"].source,
        "worker",
        Some(package("dependency", "// shared dependency", "two", None)),
    );
    // Each captured package is valid by itself; their joint dependency identity is not.
    changed.validate().unwrap();
    input.packages.insert("worker".into(), changed.clone());
    let error = prepare(
        &input,
        &request,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("Concurrent package revisions"));
    let mut restored = prepared;
    restored
        .definitions
        .flow_packages
        .insert("worker".into(), changed);
    assert!(
        format!("{:#}", restored.validate(&FixturePrimitives).unwrap_err())
            .contains("Concurrent package revisions")
    );
}

fn context_snapshot() -> CompilationSnapshot {
    use zf_context::{context::ContextStrategy, context_source};
    let mut doc = exposed_flow("worker", false);
    doc.format_version = 3;
    doc.nodes.splice(
        1..2,
        [
            flow_node(
                "context",
                "context",
                json!({"modelNode":"model","contextStrategy":"conversation","contextWindow":2048}),
            ),
            flow_node(
                "model",
                "model",
                json!({"provider":"fixture","contextNode":"context"}),
            ),
        ]
        .into_iter()
        .map(|value| serde_json::from_value(value).unwrap()),
    );
    doc.edges = serde_json::from_value(json!([{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"end"}])).unwrap();
    let source = flow_format::render(&doc, &GraphValidator::new(&FixturePrimitives)).unwrap();
    let mut input = snapshot();
    input
        .flows
        .insert("worker".into(), SourceSnapshot::capture(source.clone()));
    input
        .packages
        .insert("worker".into(), package("worker", &source, "first", None));
    for key in ["conversation", "alternate"] {
        input.programs.strategies.insert(
            key.into(),
            SourceSnapshot::capture(
                context_source::generate(&ContextStrategy::new(key, key)).unwrap(),
            ),
        );
    }
    input
}
#[test]
fn package_context_linking_and_selection_preserve_authored_source() {
    use zf_compiler::prepared::ContextSelection;
    let input = context_snapshot();
    let prepared = compile(&input).unwrap();
    assert_ne!(prepared.flows["root"].hash, input.flows["worker"].hash);
    assert_eq!(
        prepared.definitions.flow_hashes["worker"],
        input.flows["worker"].hash
    );
    prepared.validate(&FixturePrimitives).unwrap();
    let contexts = BTreeMap::from([(
        "root/model".into(),
        ContextSelection {
            key: "alternate".into(),
            hash: input.programs.strategies["alternate"].hash.clone(),
        },
    )]);
    let selected = prepare(
        &input,
        &ResolveRequest {
            flow: "worker".into(),
            entry: "main".into(),
            bridges: vec![],
        },
        &BTreeMap::new(),
        &contexts,
        &FixturePrimitives,
    )
    .unwrap();
    selected.validate(&FixturePrimitives).unwrap();
    assert_eq!(
        selected.definitions.flow_packages,
        prepared.definitions.flow_packages
    );
}
#[test]
fn valid_frozen_program_cannot_change_authored_bindings_or_strategy_identity() {
    let input = context_snapshot();
    let prepared = compile(&input).unwrap();
    for wrong_identity in [false, true] {
        let mut tampered = prepared.clone();
        let flow = tampered.flows.get_mut("root").unwrap();
        if wrong_identity {
            let mut replacement = flow.composition.clone();
            replacement.nodes[1].data.config["contextStrategy"] = json!("alternate");
            zf_compiler::programs::freeze(&mut replacement, &input.programs).unwrap();
            flow.composition.nodes[1].data.config["contextProgram"] =
                replacement.nodes[1].data.config["contextProgram"].clone();
        } else {
            flow.composition.nodes[1].data.config["contextProgram"]["window"] = json!(8192);
        }
        flow.source =
            flow_format::render(&flow.composition, &GraphValidator::new(&FixturePrimitives))
                .unwrap();
        flow.hash = zf_compiler::programs::hash(flow.source.as_bytes());
        assert!(tampered.validate(&FixturePrimitives).is_err());
    }
}

#[test]
fn runtime_requires_exact_source_when_no_context_linking_occurs() {
    let mut frozen = compile(&snapshot()).unwrap();
    let flow = frozen.flows.get_mut("root").unwrap();
    flow.source.push('\n');
    flow.hash = zf_compiler::programs::hash(flow.source.as_bytes());
    assert!(
        frozen
            .validate(&FixturePrimitives)
            .unwrap_err()
            .to_string()
            .contains("executable source differs")
    );
}

#[test]
fn standalone_package_definition_validation_requires_explicit_relative_selections() {
    use zf_compiler::prepared::ContextSelection;
    use zf_compiler::prepared_model::validate_package_definition;
    let input = context_snapshot();
    let selection = ContextSelection {
        key: "alternate".into(),
        hash: input.programs.strategies["alternate"].hash.clone(),
    };
    let selected = prepare(
        &input,
        &ResolveRequest {
            flow: "worker".into(),
            entry: "main".into(),
            bridges: vec![],
        },
        &BTreeMap::new(),
        &BTreeMap::from([("root/model".into(), selection.clone())]),
        &FixturePrimitives,
    )
    .unwrap();
    let flow = &selected.flows["root"];
    let package = &input.packages["worker"];
    let relative = BTreeMap::from([("model".into(), selection.clone())]);
    validate_package_definition(
        package,
        &flow.source,
        &flow.composition,
        &relative,
        &FixturePrimitives,
    )
    .unwrap();
    assert!(
        validate_package_definition(
            package,
            &flow.source,
            &flow.composition,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
    let unknown = BTreeMap::from([("missing".into(), selection.clone())]);
    assert!(
        validate_package_definition(
            package,
            &flow.source,
            &flow.composition,
            &unknown,
            &FixturePrimitives
        )
        .is_err()
    );
    let wrong = BTreeMap::from([(
        "model".into(),
        ContextSelection {
            key: "conversation".into(),
            ..selection.clone()
        },
    )]);
    assert!(
        validate_package_definition(
            package,
            &flow.source,
            &flow.composition,
            &wrong,
            &FixturePrimitives
        )
        .is_err()
    );
    let invalid_hash = BTreeMap::from([(
        "model".into(),
        ContextSelection {
            hash: "invalid".into(),
            ..selection
        },
    )]);
    assert!(
        validate_package_definition(
            package,
            &flow.source,
            &flow.composition,
            &invalid_hash,
            &FixturePrimitives
        )
        .is_err()
    );
    let mut changed_source = flow.source.clone();
    changed_source.push('\n');
    assert!(
        validate_package_definition(
            package,
            &changed_source,
            &flow.composition,
            &relative,
            &FixturePrimitives
        )
        .is_err()
    );
}
#[test]
fn standalone_package_definition_validation_checks_closure_identity_and_source() {
    use zf_compiler::prepared_model::validate_package_definition;
    let input = snapshot();
    let frozen = compile(&input).unwrap();
    let flow = &frozen.flows["root"];
    let original = &input.packages["worker"];
    validate_package_definition(
        original,
        &flow.source,
        &flow.composition,
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap();
    let mut corrupted = original.clone();
    corrupted
        .packages
        .get_mut(&corrupted.root)
        .unwrap()
        .files
        .insert("README.md".into(), b"bad".to_vec());
    assert!(
        validate_package_definition(
            &corrupted,
            &flow.source,
            &flow.composition,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
    let wrong_id = package("wrong", &flow.source, "first", None);
    assert!(
        validate_package_definition(
            &wrong_id,
            &flow.source,
            &flow.composition,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
    let wrong_source = flow_format::render(
        &exposed_flow("another", false),
        &GraphValidator::new(&FixturePrimitives),
    )
    .unwrap();
    assert!(
        validate_package_definition(
            original,
            &wrong_source,
            &flow.composition,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
}
