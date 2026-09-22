use serde_json::json;
use zf_flows::flow_contract;
use zf_flows::flow_source;
use zf_flows::schema::Composition;
fn flow() -> Composition {
    serde_json::from_value(json!({"formatVersion":2,"id":"worker","name":"Worker","nodes":[
 {"id":"start","position":{"x":0,"y":0},"data":{"label":"Start","kind":"start","config":{"exports":{
   "contract":{"entries":{"all":{"input":{"kind":"text"},"output":{"kind":"text"}},"publish":{"input":{"kind":"text"},"output":{"kind":"text"}}}},
   "entries":{"all":{"node":"start","inputField":"input","outputField":"response"},"publish":{"node":"output","inputField":"input","outputField":"response"}},"interactive":false
 }}}},
 {"id":"set","position":{"x":16,"y":0},"data":{"label":"Set","kind":"set","config":{"field":"output","value":"changed"}}},
 {"id":"output","position":{"x":32,"y":0},"data":{"label":"Output","kind":"output","config":{"inputField":"input","field":"response"}}},
 {"id":"end","position":{"x":48,"y":0},"data":{"label":"End","kind":"end","config":{}}}
 ],"edges":[{"id":"a","source":"start","target":"set"},{"id":"b","source":"set","target":"output"},{"id":"c","source":"output","target":"end"}]})).unwrap()
}
#[test]
fn public_entries_are_checked_and_remain_in_exact_rust() {
    let doc = flow();
    zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
    let source = flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
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
    let exports = flow_contract::validate(&doc).unwrap().unwrap();
    assert!(flow_contract::entry_input(&exports, "all", json!(42)).is_err());
    assert_eq!(
        flow_contract::entry_input(&exports, "all", json!("question")).unwrap()["input"],
        "question"
    );
    let selected = flow_contract::at_entry(&doc, "publish").unwrap();
    zf_compiler::graph_compiler::validate(&selected, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
    assert!(!selected.nodes.iter().any(|n| n.id == "set"));
    assert_eq!(
        selected
            .nodes
            .iter()
            .find(|n| n.id == "output")
            .unwrap()
            .position
            .x,
        32.0
    );
}
#[test]
fn invented_channels_nodes_and_interactivity_are_rejected() {
    let mut doc = flow();
    doc.nodes[0].data.config["exports"]["entries"]["all"]["inputField"] = json!("missing");
    assert!(
        zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
            .is_err()
    );
    let mut doc = flow();
    doc.nodes[0].data.config["exports"]["entries"]["all"]["node"] = json!("missing");
    assert!(
        zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
            .is_err()
    );
    let mut doc = flow();
    doc.nodes[1].data.kind = "input".into();
    assert!(
        zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)
            .is_err()
    );
}
