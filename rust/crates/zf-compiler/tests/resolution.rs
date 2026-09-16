use serde_json::json;
use zf_compiler::resolve::resolve;
use zf_flows::composition::{CompositionCatalog, ResolveRequest};

#[test]
fn bridge_routes_are_deterministic_and_preserve_independent_instances() {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let catalog: CompositionCatalog = serde_json::from_value(json!({
        "flows":{
            "main":{"entries":{"start":contract},"branches":{"work":{"contract":contract,"invocations":["node"]}}},
            "worker":{"entries":{"start":contract}}
        },
        "bridges":{
            "b":{"imports":{"worker":{"flow":"worker"}},"connections":{"call":{"from":{"instance":"root","port":"work"},"to":{"instance":"worker","port":"start"},"mode":"callAwait","invocation":"node"}}},
            "a":{"imports":{"worker":{"flow":"worker"}},"connections":{"call":{"from":{"instance":"root","port":"work"},"to":{"instance":"worker","port":"start"},"mode":"launch","invocation":"node"}}}
        }
    })).unwrap();
    let mut request = ResolveRequest {
        flow: "main".into(),
        entry: "start".into(),
        bridges: vec!["b".into(), "a".into(), "b".into()],
    };
    let first = resolve(&catalog, &request).unwrap();
    request.bridges = vec!["a".into(), "b".into()];
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(resolve(&catalog, &request).unwrap()).unwrap()
    );
    assert_eq!(
        first
            .instances
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["a/worker", "b/worker", "root"]
    );
    assert_eq!(first.routes["a/call"].to.instance, "a/worker");
    assert_eq!(first.routes["b/call"].to.instance, "b/worker");
    let mut invalid = catalog;
    invalid
        .bridges
        .get_mut("b")
        .unwrap()
        .connections
        .get_mut("call")
        .unwrap()
        .from
        .port = "missing".into();
    let errors = resolve(&invalid, &request).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.code == "unknown_branch" && e.path.contains("b.connections.call.from"))
    );
}

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
fn simple_model_flow() -> zf_flows::schema::Composition {
    serde_json::from_value(json!({"id":"model-loop","name":"Model loop","formatVersion":3,
        "nodes":[flow_node("start","start",json!({})),flow_node("context","context",json!({"modelNode":"model","contextStrategy":"conversation"})),flow_node("model","model",json!({"provider":"fixture","contextNode":"context"})),flow_node("end","end",json!({}))],
        "edges":[{"id":"a","source":"start","target":"context"},{"id":"b","source":"context","target":"model"},{"id":"c","source":"model","target":"end"}]
    })).unwrap()
}
#[test]
fn source_validator_checks_context_model_pairs_and_primitive_contracts() {
    use zf_compiler::graph_compiler::GraphValidator;
    use zf_flows::flow_format;
    let validator = GraphValidator::new(&FixturePrimitives);
    let doc = simple_model_flow();
    let source = flow_format::render(&doc, &validator).unwrap();
    assert_eq!(
        serde_json::to_value(flow_format::parse(&source, &validator).unwrap()).unwrap(),
        serde_json::to_value(&doc).unwrap()
    );
    let mut bypass = doc.clone();
    bypass.edges[0].target = "model".into();
    assert!(flow_format::render(&bypass, &validator).is_err());
    let mut invalid = doc;
    invalid.nodes[2].data.config["temperature"] = json!(4);
    assert!(
        flow_format::render(&invalid, &validator)
            .unwrap_err()
            .to_string()
            .contains("Invalid temperature")
    );
}

#[test]
fn lowered_loop_retains_yes_no_routes_and_alternative_arrivals() {
    use zf_compiler::plan::{PlannedEdge, lower};
    let doc: zf_flows::schema::Composition = serde_json::from_value(json!({"id":"loop","name":"Loop","formatVersion":2,
      "nodes":[flow_node("s","start",json!({})),flow_node("set","set",json!({"field":"input","value":"x","fanIn":"any"})),flow_node("if","condition",json!({"predicate":{"kind":"compare","field":"input","operator":"eq","value":"x"}})),flow_node("again","set",json!({"field":"input","value":"y"})),flow_node("e","end",json!({}))],
      "edges":[{"id":"a","source":"s","target":"set"},{"id":"b","source":"set","target":"if"},{"id":"c","source":"if","target":"again","sourceHandle":"true"},{"id":"d","source":"if","target":"e","sourceHandle":"false"},{"id":"e","source":"again","target":"set"}]
    })).unwrap();
    let plan = lower(&doc, &FixturePrimitives).unwrap();
    assert_eq!(plan.nodes().len(), 3);
    assert!(plan.edges().iter().any(|edge| matches!(edge, PlannedEdge::Conditional { source, field, expected, yes, no } if source=="if" && field=="__zedflow:condition:if" && expected==&json!(true) && yes=="again" && no=="__end__")));
    assert!(plan.edges().iter().any(|edge| matches!(edge, PlannedEdge::Alternative {source,target} if source=="again" && target=="set")));
    assert_eq!(
        plan.revision().unwrap(),
        lower(&doc, &FixturePrimitives).unwrap().revision().unwrap()
    );
    let mut changed = doc;
    changed.nodes[1].data.config["value"] = json!("z");
    assert_ne!(
        plan.revision().unwrap(),
        lower(&changed, &FixturePrimitives)
            .unwrap()
            .revision()
            .unwrap()
    );
}

#[test]
fn context_linking_uses_only_captured_sources_and_rejects_missing_bindings() {
    use zf_compiler::programs::{ProgramSources, SourceSnapshot, freeze};
    use zf_context::{context::ContextStrategy, context_source};
    let strategy = ContextStrategy::new("conversation", "Conversation")
        .require("input", zf_core::types::DataType::Text);
    let source = context_source::generate(&strategy).unwrap();
    let mut sources = ProgramSources::default();
    sources.strategies.insert(
        "conversation".into(),
        SourceSnapshot::capture(source.clone()),
    );
    let mut doc = simple_model_flow();
    doc.nodes[1].data.config["contextBindings"] = json!({"input":{"kind":"state","field":"input"}});
    let deps = freeze(&mut doc, &sources).unwrap();
    assert_eq!(deps.len(), 1);
    let frozen = doc.nodes[1].data.config["contextProgram"].clone();
    assert_eq!(frozen["source"], source);
    sources.strategies.insert(
        "conversation".into(),
        SourceSnapshot::capture("external invalid edit".into()),
    );
    zf_context::frozen_context::validate_frozen(&frozen).unwrap();
    assert!(freeze(&mut simple_model_flow(), &sources).is_err());
    sources
        .strategies
        .insert("conversation".into(), SourceSnapshot::capture(source));
    assert!(
        freeze(&mut simple_model_flow(), &sources)
            .unwrap_err()
            .to_string()
            .contains("sans binding")
    );
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
#[test]
fn captured_composition_keeps_exact_sources_pins_and_rejects_stale_selection() {
    use std::collections::BTreeMap;
    use zf_compiler::{
        graph_compiler::GraphValidator,
        prepared::{CompilationSnapshot, prepare},
        programs::SourceSnapshot,
    };
    use zf_flows::{bridge_source, composition::*, flow_format};
    let validator = GraphValidator::new(&FixturePrimitives);
    let mut snapshot = CompilationSnapshot::default();
    for (id, root) in [("pilot", true), ("worker", false)] {
        snapshot.flows.insert(
            id.into(),
            SourceSnapshot::capture(
                flow_format::render(&exposed_flow(id, root), &validator).unwrap(),
            ),
        );
    }
    let bridge = BridgeDefinition::new().import("worker", "worker").connect(
        "work",
        Connection::new(
            Endpoint::new("root", "work"),
            Endpoint::new("worker", "main"),
            RouteMode::CallAwait,
            InvocationKind::Node,
        ),
    );
    snapshot.bridges.insert(
        "work".into(),
        SourceSnapshot::capture(bridge_source::generate(&bridge).unwrap()),
    );
    let request = ResolveRequest {
        flow: "pilot".into(),
        entry: "main".into(),
        bridges: vec!["work".into()],
    };
    let expected = BTreeMap::from([("pilot".into(), snapshot.flows["pilot"].hash.clone())]);
    let prepared = prepare(
        &snapshot,
        &request,
        &expected,
        &BTreeMap::new(),
        &FixturePrimitives,
    )
    .unwrap();
    assert!(!prepared.interactive());
    assert_eq!(
        prepared.flows["root"].source,
        snapshot.flows["pilot"].source
    );
    assert_eq!(
        prepared.definitions.bridge_sources["work"],
        snapshot.bridges["work"].source
    );
    prepared.validate(&FixturePrimitives).unwrap();
    snapshot.flows.insert(
        "pilot".into(),
        SourceSnapshot::capture(
            flow_format::render(&exposed_flow("modified", true), &validator).unwrap(),
        ),
    );
    assert!(
        prepare(
            &snapshot,
            &request,
            &expected,
            &BTreeMap::new(),
            &FixturePrimitives
        )
        .is_err()
    );
    prepared.validate(&FixturePrimitives).unwrap();
    let mut tampered = prepared;
    tampered.flows.get_mut("root").unwrap().source.push(' ');
    assert!(tampered.validate(&FixturePrimitives).is_err());
}

#[test]
fn compilation_exposes_a_stable_plan_and_structured_resolution_diagnostics() {
    use zf_compiler::{
        compiler::{CompileRequest, compile},
        graph_compiler::GraphValidator,
        prepared::CompilationSnapshot,
        programs::SourceSnapshot,
    };
    let validator = GraphValidator::new(&FixturePrimitives);
    let mut snapshot = CompilationSnapshot::default();
    snapshot.flows.insert(
        "worker".into(),
        SourceSnapshot::capture(
            zf_flows::flow_format::render(&exposed_flow("worker", false), &validator).unwrap(),
        ),
    );
    let mut request = CompileRequest::new(ResolveRequest {
        flow: "worker".into(),
        entry: "main".into(),
        bridges: vec![],
    });
    let a = compile(&snapshot, &request, &FixturePrimitives).unwrap();
    let b = compile(&snapshot, &request, &FixturePrimitives).unwrap();
    assert_eq!(a.revision(), b.revision());
    assert_eq!(a.graphs()["root"].nodes().len(), 1);
    request.entry.bridges.push("missing".into());
    let errors = compile(&snapshot, &request, &FixturePrimitives).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|d| d.code == "missing_bridge" && !d.path.is_empty())
    );
}

#[test]
fn captured_dependencies_are_logical_and_override_failure_does_not_mutate_the_flow() {
    use zf_compiler::programs::{self, ProgramSources, SourceKind, SourceOverride, SourceSnapshot};
    use zf_context::{context::ContextStrategy, context_source};
    let source =
        context_source::generate(&ContextStrategy::new("conversation", "Conversation")).unwrap();
    let captured = SourceSnapshot::capture(source.clone());
    let mut sources = ProgramSources::default();
    sources
        .strategies
        .insert("conversation".into(), captured.clone());
    let mut doc = simple_model_flow();
    programs::freeze(&mut doc, &sources).unwrap();
    let dependencies = programs::captured_sources(&doc).unwrap();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].key, "conversation");
    assert_eq!(dependencies[0].kind, SourceKind::Strategy);
    assert_eq!(dependencies[0].hash, captured.hash);
    let before = serde_json::to_value(&doc).unwrap();
    let bad = SourceOverride {
        kind: SourceKind::Strategy,
        key: "conversation".into(),
        source,
        expected_hash: Some("0".repeat(64)),
    };
    assert!(programs::freeze_with_overrides(&mut doc, &sources, &[bad]).is_err());
    assert_eq!(serde_json::to_value(doc).unwrap(), before);
}

// Conservation of the existing pure resolver suite; execution tests stay with runtime.
mod historical_resolution {
    use serde_json::{Value, json};
    use zf_compiler::resolve::{RuntimeGraph, resolve};
    use zf_flows::composition::{CompositionCatalog, ResolveRequest};

    fn endpoint(instance: &str, port: &str) -> Value {
        json!({"instance":instance,"port":port})
    }
    fn read() -> Value {
        json!({"read":true,"write":false})
    }
    fn read_write() -> Value {
        json!({"read":true,"write":true})
    }
    fn named(name: &str) -> Value {
        json!({"kind":"named","name":name})
    }
    fn record(fields: Value) -> Value {
        json!({"kind":"record","fields":fields})
    }
    fn contract() -> Value {
        json!({"input":record(json!({"question":{"kind":"text"}})),"output":record(json!({"answer":{"kind":"text"}}))})
    }
    fn catalog() -> Value {
        json!({
            "types":{"Notes":{"kind":"list","item":{"kind":"text"}}},
            "flows":{
                "pilot":{
                    "entries":{"start":contract()},
                    "branches":{"research":{"contract":contract(),"invocations":["node","tool","condition"]}},
                    "data":{"notes":{"dataType":named("Notes"),"permissions":read_write()}},
                    "inferenceNodes":{"agent":{"model":{"kind":"fixed","provider":"fixture","model":"fixture"}}}
                },
                "research":{
                    "entries":{"start":contract()},
                    "branches":{"again":{"contract":contract(),"invocations":["node"]}},
                    "requires":{"notes":{"dataType":named("Notes"),"permissions":read()}},
                    "inferenceNodes":{"agent":{"model":{"kind":"runtime"},"contextStrategy":"research-context","resources":["notes"],"capabilities":["read"]}}
                }
            },
            "bridges":{}
        })
    }
    fn bridge() -> Value {
        json!({
            "imports":{"worker":{"flow":"research"}},
            "connections":{"ask":{"from":endpoint("root","research"),"to":endpoint("worker","start"),"mode":"callAwait","invocation":"node"}},
            "bindings":{"notes":{"from":endpoint("root","notes"),"to":endpoint("worker","notes"),"permissions":read()}}
        })
    }
    fn request(bridges: &[&str]) -> ResolveRequest {
        ResolveRequest {
            flow: "pilot".into(),
            entry: "start".into(),
            bridges: bridges.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn resolve_value(value: Value, selected: &[&str]) -> RuntimeGraph {
        resolve(&serde_json::from_value(value).unwrap(), &request(selected)).unwrap()
    }
    fn errors(value: Value, selected: &[&str]) -> Vec<String> {
        resolve(&serde_json::from_value(value).unwrap(), &request(selected))
            .unwrap_err()
            .into_iter()
            .map(|diagnostic| {
                assert!(!diagnostic.path.is_empty());
                assert!(!diagnostic.message.is_empty());
                diagnostic.code
            })
            .collect()
    }

    #[test]
    fn independent_imports_preserve_named_instances_and_runtime_selectable_inferences() {
        let mut source = catalog();
        source["bridges"] = json!({"a":bridge(),"b":bridge()});
        let graph = resolve_value(source.clone(), &["b", "a", "b"]);
        assert_eq!(
            graph
                .instances
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["a/worker", "b/worker", "root"]
        );
        assert_eq!(graph.routes.len(), 2);
        assert_eq!(graph.routes["a/ask"].bridge, "a");
        assert_eq!(graph.routes["a/ask"].to.instance, "a/worker");
        assert_eq!(graph.routes["b/ask"].to.instance, "b/worker");
        assert_eq!(graph.data_bindings["a/notes"].from.instance, "root");
        assert_eq!(graph.data_bindings["b/notes"].from.instance, "root");
        assert_eq!(graph.inferences.len(), 3);
        let serialized = serde_json::to_value(&graph).unwrap();
        assert_eq!(
            serialized["inferences"]["a/worker/agent"]["definition"]["model"]["kind"],
            "runtime"
        );
        assert_eq!(
            serialized["inferences"]["root/agent"]["definition"]["model"]["kind"],
            "fixed"
        );
        assert_eq!(
            serialized["inferences"]["b/worker/agent"]["definition"]["contextStrategy"],
            "research-context"
        );
        assert_eq!(
            serde_json::to_value(resolve_value(source, &["a", "b"])).unwrap(),
            serialized
        );
        let roundtrip: RuntimeGraph = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), serialized);
    }

    #[test]
    fn dependencies_are_closed_and_explicit_reuse_has_one_instance_and_one_inference() {
        let mut source = catalog();
        let mut shared = bridge();
        shared["connections"] = json!({});
        let mut dependent = bridge();
        dependent["requires"] = json!(["shared"]);
        dependent["imports"]["worker"]["reuse"] = json!("shared/worker");
        dependent["bindings"] = json!({});
        source["bridges"] = json!({"shared":shared,"use":dependent});
        let graph = resolve_value(source, &["use"]);
        assert_eq!(graph.bridges.len(), 2);
        assert_eq!(graph.instances.len(), 2);
        assert_eq!(graph.aliases["use/worker"], "shared/worker");
        assert_eq!(graph.routes["use/ask"].to.instance, "shared/worker");
        assert_eq!(graph.inferences.len(), 2);
        assert_eq!(graph.data_bindings.len(), 1);
    }

    #[test]
    fn calls_launches_handoffs_and_invocation_surfaces_remain_distinct_routes() {
        let mut source = catalog();
        let mut routes = bridge();
        routes["connections"] = json!({
            "tool-call":{"from":endpoint("root","research"),"to":endpoint("worker","start"),"mode":"callAwait","invocation":"tool","toolName":"research"},
            "background":{"from":endpoint("root","research"),"to":endpoint("worker","start"),"mode":"launch","invocation":"node"},
            "transfer":{"from":endpoint("root","research"),"to":endpoint("worker","start"),"mode":"handoff","invocation":"condition","condition":{"kind":"compare","field":"ready","operator":"eq","value":true}},
            "loop":{"from":endpoint("worker","again"),"to":endpoint("worker","start"),"mode":"callAwait","invocation":"node"}
        });
        source["bridges"]["routes"] = routes;
        let graph = serde_json::to_value(resolve_value(source, &["routes"])).unwrap();
        assert_eq!(graph["bridges"].as_object().unwrap().len(), 1);
        assert_eq!(graph["routes"].as_object().unwrap().len(), 4);
        assert_eq!(graph["routes"]["routes/tool-call"]["mode"], "callAwait");
        assert_eq!(graph["routes"]["routes/background"]["mode"], "launch");
        assert_eq!(graph["routes"]["routes/transfer"]["mode"], "handoff");
        assert_eq!(graph["routes"]["routes/tool-call"]["toolName"], "research");
        assert_eq!(
            graph["routes"]["routes/transfer"]["condition"]["value"],
            true
        );
    }

    #[test]
    fn missing_dependencies_and_cycles_are_reported_without_resolving_inactive_bridges() {
        let mut source = catalog();
        source["bridges"]["unused"] =
            json!({"requires":["absent"],"imports":{"broken":{"flow":"absent"}}});
        assert_eq!(resolve_value(source.clone(), &[]).instances.len(), 1);
        assert!(errors(source.clone(), &["unused"]).contains(&"missing_bridge".into()));
        assert!(errors(source.clone(), &["unused"]).contains(&"missing_flow".into()));
        source["bridges"] = json!({"a":{"requires":["b"]},"b":{"requires":["a"]}});
        assert!(errors(source, &["a"]).contains(&"bridge_cycle".into()));
        let source: CompositionCatalog = serde_json::from_value(catalog()).unwrap();
        let mut selected = request(&[]);
        selected.entry = "missing".into();
        assert_eq!(
            resolve(&source, &selected).unwrap_err()[0].code,
            "unknown_entry"
        );
        selected.flow = "missing".into();
        assert_eq!(
            resolve(&source, &selected).unwrap_err()[0].code,
            "missing_flow"
        );
    }

    #[test]
    fn aliases_cannot_leak_other_bridge_instances_or_reuse_a_different_flow() {
        let mut source = catalog();
        source["bridges"] = json!({"a":bridge(),"b":bridge()});
        source["bridges"]["b"]["imports"]["worker"]["reuse"] = json!("a/worker");
        assert!(errors(source.clone(), &["a", "b"]).contains(&"reuse_scope".into()));
        source["bridges"]["b"]["requires"] = json!(["a"]);
        source["bridges"]["b"]["imports"]["worker"]["flow"] = json!("pilot");
        assert!(errors(source.clone(), &["b"]).contains(&"reuse_flow_mismatch".into()));
        source["bridges"]["b"]["imports"]["worker"] = json!({"flow":"research"});
        source["bridges"]["b"]["connections"]["ask"]["to"]["instance"] = json!("a/worker");
        assert!(errors(source, &["b"]).contains(&"alias_scope".into()));
    }

    #[test]
    fn reuse_cycles_reserved_aliases_and_unknown_ports_have_specific_diagnostics() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["bridges"]["a"]["imports"] = json!({
            "worker":{"flow":"research","reuse":"a/other"},
            "other":{"flow":"research","reuse":"a/worker"},
            "root":{"flow":"pilot"}
        });
        let found = errors(source, &["a"]);
        assert!(found.contains(&"reuse_cycle".into()));
        assert!(found.contains(&"reserved_alias".into()));
        for (field, value, expected) in [
            ("from", endpoint("absent", "research"), "unknown_alias"),
            ("from", endpoint("root", "private"), "unknown_branch"),
            ("to", endpoint("worker", "private"), "unknown_entry"),
        ] {
            let mut source = catalog();
            source["bridges"]["a"] = bridge();
            source["bridges"]["a"]["connections"]["ask"][field] = value;
            assert!(errors(source, &["a"]).contains(&expected.into()));
        }
    }

    #[test]
    fn route_input_and_awaited_output_are_directionally_checked() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["flows"]["research"]["entries"]["start"]["input"] =
            record(json!({"question":{"kind":"number"}}));
        source["flows"]["research"]["entries"]["start"]["output"] = json!({"kind":"boolean"});
        let found = errors(source.clone(), &["a"]);
        assert!(found.contains(&"route_input_type".into()));
        assert!(found.contains(&"route_output_type".into()));
        source["flows"]["research"]["entries"]["start"]["input"] = contract()["input"].clone();
        source["bridges"]["a"]["connections"]["ask"]["mode"] = json!("launch");
        resolve_value(source, &["a"]);
    }

    #[test]
    fn dataset_grants_cannot_escalate_and_mutable_record_bindings_are_invariant() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["flows"]["pilot"]["data"]["notes"]["permissions"] = read();
        source["flows"]["research"]["requires"]["notes"]["permissions"] = read_write();
        source["bridges"]["a"]["bindings"]["notes"]["permissions"] = read_write();
        assert!(errors(source.clone(), &["a"]).contains(&"binding_permissions".into()));
        source["flows"]["pilot"]["data"]["notes"]["permissions"] = read_write();
        source["flows"]["pilot"]["data"]["notes"]["dataType"] =
            record(json!({"title":{"kind":"text"},"owner":{"kind":"text"}}));
        source["flows"]["research"]["requires"]["notes"]["dataType"] =
            record(json!({"title":{"kind":"text"}}));
        assert!(errors(source.clone(), &["a"]).contains(&"binding_type".into()));
        source["flows"]["research"]["requires"]["notes"]["permissions"] = read();
        source["bridges"]["a"]["bindings"]["notes"]["permissions"] = read();
        resolve_value(source, &["a"]);
    }

    #[test]
    fn write_only_grants_are_rejected_at_every_contract_boundary_without_normalization() {
        for pointer in [
            "/flows/pilot/data/notes/permissions",
            "/flows/research/requires/notes/permissions",
            "/bridges/a/bindings/notes/permissions",
        ] {
            let mut source = catalog();
            source["bridges"]["a"] = bridge();
            *source.pointer_mut(pointer).unwrap() = json!({"read":false,"write":true});
            let catalog: CompositionCatalog = serde_json::from_value(source.clone()).unwrap();
            let found = resolve(&catalog, &request(&["a"])).unwrap_err();
            assert!(
                found
                    .iter()
                    .any(|error| error.code == "permissions_unsupported")
            );
            assert_eq!(
                serde_json::to_value(&catalog).unwrap().pointer(pointer),
                source.pointer(pointer)
            );
        }
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["flows"]["research"]["requires"]["notes"]["permissions"] = read_write();
        source["bridges"]["a"]["bindings"]["notes"]["permissions"] = read_write();
        resolve_value(source, &["a"]);
    }

    #[test]
    fn required_data_is_bound_once_even_when_an_instance_is_reused() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["bridges"]["a"]["bindings"] = json!({});
        assert!(errors(source.clone(), &["a"]).contains(&"unbound_data".into()));
        source["flows"]["research"]["requires"]["notes"]["optional"] = json!(true);
        resolve_value(source, &["a"]);
        let mut source = catalog();
        source["bridges"] = json!({"a":bridge(),"b":bridge()});
        source["bridges"]["b"]["requires"] = json!(["a"]);
        source["bridges"]["b"]["imports"]["worker"]["reuse"] = json!("a/worker");
        assert!(errors(source, &["b"]).contains(&"duplicate_binding".into()));
    }

    #[test]
    fn unknown_nominal_types_and_inference_dependencies_are_not_silently_accepted() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["flows"]["research"]["requires"]["notes"]["dataType"] = named("Absent");
        source["flows"]["research"]["inferenceNodes"]["agent"]["resources"] =
            json!(["private-unknown"]);
        let found = errors(source, &["a"]);
        assert!(found.contains(&"unknown_type".into()));
        assert!(found.contains(&"unknown_resource".into()));
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["types"]["OtherNotes"] = source["types"]["Notes"].clone();
        source["flows"]["research"]["requires"]["notes"]["dataType"] = named("OtherNotes");
        assert!(errors(source, &["a"]).contains(&"binding_type".into()));
    }

    #[test]
    fn tool_names_conditions_and_branch_permissions_are_validated() {
        let mut source = catalog();
        source["bridges"]["a"] = bridge();
        source["bridges"]["a"]["connections"]["ask"]["invocation"] = json!("tool");
        assert!(errors(source.clone(), &["a"]).contains(&"missing_tool_name".into()));
        source["bridges"]["a"]["connections"]["ask"]["toolName"] = json!("research");
        source["bridges"]["a"]["connections"]["duplicate"] =
            source["bridges"]["a"]["connections"]["ask"].clone();
        assert!(errors(source.clone(), &["a"]).contains(&"duplicate_tool".into()));
        source["bridges"]["a"]["connections"] = bridge()["connections"].clone();
        source["bridges"]["a"]["connections"]["ask"]["invocation"] = json!("condition");
        assert!(errors(source.clone(), &["a"]).contains(&"missing_condition".into()));
        source["bridges"]["a"]["connections"]["ask"]["condition"] =
            json!({"kind":"all","items":[]});
        source["flows"]["pilot"]["branches"]["research"]["invocations"] = json!(["node"]);
        let found = errors(source, &["a"]);
        assert!(found.contains(&"invalid_condition".into()));
        assert!(found.contains(&"invocation_not_allowed".into()));
    }
}

#[test]
fn native_all_join_keeps_parallel_work_safe_before_a_single_wait() {
    let mut doc: zf_flows::schema::Composition = serde_json::from_value(json!({"id":"join","name":"Join","formatVersion":2,
      "nodes":[flow_node("s","start",json!({})),flow_node("a","set",json!({"field":"input","value":1})),flow_node("b","set",json!({"field":"output","value":2})),flow_node("join","set",json!({"field":"response","value":"ready"})),flow_node("ask","input",json!({})),flow_node("e","end",json!({}))],
      "edges":[{"id":"a","source":"s","target":"a"},{"id":"b","source":"s","target":"b"},{"id":"c","source":"a","target":"join"},{"id":"d","source":"b","target":"join"},{"id":"e","source":"join","target":"ask"},{"id":"f","source":"ask","target":"e"}]
    })).unwrap();
    let plan = zf_compiler::plan::lower(&doc, &FixturePrimitives).unwrap();
    assert_eq!(plan.answer_paths(), ["ask"]);
    doc.nodes[3].data.config["fanIn"] = json!("any");
    let error = zf_compiler::plan::lower(&doc, &FixturePrimitives).unwrap_err();
    assert!(error.to_string().contains("attentes parallèles"));
    doc.nodes[3].data.config["fanIn"] = json!("all");
    let parent: zf_flows::schema::Composition=serde_json::from_value(json!({"id":"nested","name":"Nested","formatVersion":2,"nodes":[flow_node("s","start",json!({})),flow_node("child","subgraph",json!({"composition":doc})),flow_node("e","end",json!({}))],"edges":[{"id":"a","source":"s","target":"child"},{"id":"b","source":"child","target":"e"}]})).unwrap();
    let nested = zf_compiler::plan::lower(&parent, &FixturePrimitives).unwrap();
    assert_eq!(nested.answer_paths(), ["child/ask"]);
    assert_eq!(
        nested.nodes()[0].child.as_ref().unwrap().answer_paths(),
        ["ask"]
    );
}

#[test]
fn context_programs_follow_only_explicitly_accepted_reference_ancestry() {
    use zf_compiler::programs::{self, ProgramSources, SourceSnapshot};
    use zf_context::{context::ContextStrategy, context_source};
    let source =
        context_source::generate(&ContextStrategy::new("conversation", "New name")).unwrap();
    let previous =
        context_source::generate(&ContextStrategy::new("conversation", "Old name")).unwrap();
    let old_hash = programs::hash(previous.as_bytes());
    let mut captured = SourceSnapshot::capture(source.clone());
    let mut sources = ProgramSources::default();
    sources
        .strategies
        .insert("conversation".into(), captured.clone());
    let mut doc = simple_model_flow();
    doc.nodes[1].data.config["contextStrategy"] = json!({"key":"conversation","hash":old_hash});
    assert!(programs::freeze(&mut doc, &sources).is_err());
    captured.accepted_references.insert(old_hash);
    sources.strategies.insert("conversation".into(), captured);
    programs::freeze(&mut doc, &sources).unwrap();
    assert_eq!(doc.nodes[1].data.config["contextProgram"]["source"], source);
    sources
        .strategies
        .get_mut("conversation")
        .unwrap()
        .source
        .push(' ');
    assert!(programs::freeze(&mut doc, &sources).is_err());
}

#[test]
fn compiled_entries_are_projected_without_replacing_frozen_authoring_sources() {
    use zf_compiler::{
        compiler::{CompileRequest, compile},
        graph_compiler::GraphValidator,
        prepared::CompilationSnapshot,
        programs::SourceSnapshot,
    };
    let mut value = serde_json::to_value(exposed_flow("worker", false)).unwrap();
    value["nodes"]
        .as_array_mut()
        .unwrap()
        .insert(2, flow_node("out", "output", json!({"text":"{{input}}"})));
    value["edges"] = json!([{"id":"a","source":"start","target":"action"},{"id":"b","source":"action","target":"out"},{"id":"c","source":"out","target":"end"}]);
    value["nodes"][0]["data"]["config"]["exports"]["contract"]["entries"]["secondary"] =
        json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    value["nodes"][0]["data"]["config"]["exports"]["entries"]["secondary"] =
        json!({"node":"out","inputField":"input","outputField":"response"});
    let doc = serde_json::from_value(value).unwrap();
    let source =
        zf_flows::flow_format::render(&doc, &GraphValidator::new(&FixturePrimitives)).unwrap();
    let mut snapshot = CompilationSnapshot::default();
    snapshot
        .flows
        .insert("worker".into(), SourceSnapshot::capture(source.clone()));
    let request = CompileRequest::new(ResolveRequest {
        flow: "worker".into(),
        entry: "secondary".into(),
        bridges: vec![],
    });
    let compiled = compile(&snapshot, &request, &FixturePrimitives).unwrap();
    let secondary = compiled.entry("root", "secondary").unwrap();
    assert_eq!(
        secondary
            .graph()
            .nodes()
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["out"]
    );
    assert!(
        !secondary
            .composition()
            .nodes
            .iter()
            .any(|n| n.id == "action")
    );
    assert!(
        compiled
            .entry("root", "main")
            .unwrap()
            .graph()
            .nodes()
            .iter()
            .any(|n| n.id == "action")
    );
    assert!(compiled.entry("root", "missing").is_none());
    assert!(compiled.entry("missing", "secondary").is_none());
    assert_eq!(compiled.prepared().flows["root"].source, source);
    assert!(
        compiled.prepared().flows["root"]
            .composition
            .nodes
            .iter()
            .any(|n| n.id == "action")
    );
}
