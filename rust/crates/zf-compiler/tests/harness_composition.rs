use serde_json::{Value, json};
use zf_compiler::resolve::RuntimeGraph;
use zf_compiler::resolve::resolve;
use zf_flows::composition::CompositionCatalog;
use zf_flows::composition::ResolveRequest;

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
    source["bridges"]["a"]["connections"]["ask"]["condition"] = json!({"kind":"all","items":[]});
    source["flows"]["pilot"]["branches"]["research"]["invocations"] = json!(["node"]);
    let found = errors(source, &["a"]);
    assert!(found.contains(&"invalid_condition".into()));
    assert!(found.contains(&"invocation_not_allowed".into()));
}
