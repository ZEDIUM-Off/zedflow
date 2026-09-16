use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use tempfile::TempDir;
use zf_compiler::{graph_compiler::GraphValidator, prepared::ContextSelection};
use zf_context::context::{
    ContextBlock, ContextExpr, ContextStrategy, FragmentFormat, FragmentRole,
};
use zf_core::types::DataType;
use zf_execution::preparation::{RuntimeSelection, prepare};
use zf_flows::{
    composition::{BridgeDefinition, Connection, Endpoint, InvocationKind, RouteMode},
    flow_source,
    schema::Composition,
};
use zf_runtime::materialize::RuntimePrimitives;
use zf_storage::{
    bridge_store::BridgeStore,
    context_store::ContextStore,
    flow_store::{FlowFile, FlowStore},
    workspaces::{Workspace, path_id},
};

struct Fixture {
    _root: TempDir,
    workspace: Workspace,
    flows: FlowStore,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        std::fs::create_dir_all(workspace.join(".zedflow/flows")).unwrap();
        std::fs::create_dir(&home).unwrap();
        Self {
            workspace: Workspace {
                id: path_id(&workspace),
                name: "Preparation fixture".into(),
                path: workspace,
                open: true,
            },
            flows: FlowStore::new(home, Arc::new(GraphValidator::new(&RuntimePrimitives))),
            _root: root,
        }
    }
    async fn flow(&self, doc: &Composition) -> FlowFile {
        let path = self
            .workspace
            .path
            .join(format!(".zedflow/flows/{}.rs", doc.id));
        let source = flow_source::render(doc, &GraphValidator::new(&RuntimePrimitives)).unwrap();
        std::fs::write(&path, source).unwrap();
        self.flows
            .get(&self.workspace, &path_id(&path))
            .await
            .unwrap()
    }
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn flow(id: &str, kind: &str, config: Value) -> Composition {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"output"}},"interactive":false});
    if kind == "route" {
        exports["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["node"]}});
        exports["branches"] = json!({"work":"action"});
    }
    serde_json::from_value(json!({"formatVersion":3,"id":id,"name":id,"nodes":[
        node("start","start",json!({"exports":exports})),
        node("action",kind,config),node("end","end",json!({}))
    ],"edges":[{"id":"a","source":"start","target":"action"},{"id":"b","source":"action","target":"end"}]})).unwrap()
}
fn selection(file: &FlowFile) -> RuntimeSelection {
    RuntimeSelection {
        flow: file.key.clone(),
        entry: "main".into(),
        bridges: vec![],
        flow_hashes: BTreeMap::from([(file.key.clone(), file.hash.clone())]),
        bridge_hashes: BTreeMap::new(),
        contexts: BTreeMap::new(),
    }
}
fn strategy(text: &str) -> ContextStrategy {
    ContextStrategy::new("chosen", "Chosen").with_program(vec![ContextBlock::emit(
        "instruction",
        FragmentRole::Instruction,
        FragmentFormat::Text,
        ContextExpr::literal(DataType::Text, json!(text)),
    )])
}

#[tokio::test]
async fn captures_exact_selected_flow_and_bridge_sources_and_rejects_stale_pins() {
    let f = Fixture::new();
    let root = f
        .flow(&flow("root", "route", json!({"branch":"work"})))
        .await;
    let child = f
        .flow(&flow(
            "child",
            "set",
            json!({"field":"output","value":"done"}),
        ))
        .await;
    let bridges = BridgeStore::new(f.workspace.path.clone()).unwrap();
    let bridge = bridges
        .save(
            "work",
            &BridgeDefinition::new().import("child", &child.key).connect(
                "work",
                Connection::new(
                    Endpoint::new("root", "work"),
                    Endpoint::new("child", "main"),
                    RouteMode::CallAwait,
                    InvocationKind::Node,
                ),
            ),
            None,
        )
        .await
        .unwrap();
    let mut request = selection(&root);
    request.bridges.push("work".into());
    request
        .bridge_hashes
        .insert("work".into(), bridge.hash.clone());
    let prepared = prepare(&f.flows, &f.workspace, &request).await.unwrap();
    assert_eq!(prepared.flows["root"].source, root.source.unwrap());
    assert_eq!(prepared.definitions.flow_hashes[&root.key], root.hash);
    assert_eq!(
        prepared.definitions.bridge_sources["work"],
        bridge.source.unwrap()
    );
    assert_eq!(prepared.definitions.bridge_hashes["work"], bridge.hash);
    request.bridge_hashes.insert("work".into(), "0".repeat(64));
    assert!(
        prepare(&f.flows, &f.workspace, &request)
            .await
            .unwrap_err()
            .to_string()
            .contains("Bridge changed")
    );
    request.bridge_hashes.clear();
    request.flow_hashes.insert(root.key, "0".repeat(64));
    assert!(
        prepare(&f.flows, &f.workspace, &request)
            .await
            .unwrap_err()
            .to_string()
            .contains("Flow changed")
    );
}

#[tokio::test]
async fn overlay_is_applied_before_acquiring_missing_original_strategy() {
    let f = Fixture::new();
    let file = f
        .flow(&flow(
            "agent",
            "agent",
            json!({"provider":"fixture","contextStrategy":"missing","contextBindings":{}}),
        ))
        .await;
    let context = ContextStore::new(f.workspace.path.clone())
        .save(&strategy("selected"), None)
        .await
        .unwrap();
    let mut request = selection(&file);
    request.contexts.insert(
        "root/action".into(),
        ContextSelection {
            key: "chosen".into(),
            hash: context.hash.clone(),
        },
    );
    let runtime = prepare(&f.flows, &f.workspace, &request).await.unwrap();
    assert_eq!(
        runtime.flows["root"].composition.nodes[1].data.config["contextProgram"]["hash"],
        context.hash
    );
    assert_eq!(
        runtime.definitions.context_selections["root/action"].hash,
        context.hash
    );
    assert_eq!(runtime.definitions.flow_hashes[&file.key], file.hash);
    request.contexts.get_mut("root/action").unwrap().hash = "0".repeat(64);
    assert!(prepare(&f.flows, &f.workspace, &request).await.is_err());
}

#[tokio::test]
async fn accepted_strategy_ancestor_follows_head_but_external_edits_are_rejected() {
    let f = Fixture::new();
    let contexts = ContextStore::new(f.workspace.path.clone());
    let first = contexts.save(&strategy("first"), None).await.unwrap();
    let file = f.flow(&flow("agent", "agent", json!({"provider":"fixture","contextStrategy":{"key":"chosen","hash":first.hash},"contextBindings":{}}))).await;
    let second = contexts
        .save(&strategy("second"), Some(&first.hash))
        .await
        .unwrap();
    let runtime = prepare(&f.flows, &f.workspace, &selection(&file))
        .await
        .unwrap();
    assert_eq!(
        runtime.flows["root"].composition.nodes[1].data.config["contextProgram"]["hash"],
        second.hash
    );
    assert_eq!(runtime.definitions.flow_hashes[&file.key], file.hash);
    std::fs::write(second.path, "external edit").unwrap();
    assert!(
        prepare(&f.flows, &f.workspace, &selection(&file))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unused_invalid_programs_and_flow_sources_do_not_block_selected_flow() {
    let f = Fixture::new();
    let file = f
        .flow(&flow(
            "selected",
            "set",
            json!({"field":"output","value":"done"}),
        ))
        .await;
    f.flow(&flow(
        "unused",
        "agent",
        json!({"provider":"fixture","contextStrategy":"missing","contextBindings":{}}),
    ))
    .await;
    std::fs::write(
        f.workspace.path.join(".zedflow/flows/broken.rs"),
        "malformed",
    )
    .unwrap();
    std::fs::create_dir_all(f.workspace.path.join(".zedflow/context")).unwrap();
    std::fs::write(
        f.workspace.path.join(".zedflow/context/broken.rs"),
        "malformed",
    )
    .unwrap();
    let runtime = prepare(&f.flows, &f.workspace, &selection(&file))
        .await
        .unwrap();
    assert_eq!(runtime.flows.len(), 1);
    assert_eq!(runtime.flows["root"].key, file.key);
}

#[tokio::test]
async fn selected_working_directory_is_checked_without_changing_process_cwd() {
    let f = Fixture::new();
    let cwd = std::env::current_dir().unwrap();
    let mut doc = flow("directory", "set", json!({"field":"output","value":"done"}));
    doc.settings.working_directory = Some("missing-directory".into());
    let file = f.flow(&doc).await;
    assert!(
        prepare(&f.flows, &f.workspace, &selection(&file))
            .await
            .unwrap_err()
            .to_string()
            .contains("inaccessible")
    );
    assert_eq!(std::env::current_dir().unwrap(), cwd);
}

#[tokio::test]
async fn unselected_public_flow_contributes_shared_types() {
    let f = Fixture::new();
    let file = f
        .flow(&flow(
            "selected",
            "set",
            json!({"field":"output","value":"done"}),
        ))
        .await;
    let mut donor = flow("types", "set", json!({"field":"output","value":"done"}));
    donor.nodes[0].data.config["exports"]["types"] = json!({"SharedText":{"kind":"text"}});
    f.flow(&donor).await;
    let runtime = prepare(&f.flows, &f.workspace, &selection(&file))
        .await
        .unwrap();
    assert!(runtime.graph.types.contains_key("SharedText"));
    assert_eq!(runtime.flows.len(), 1);
}

#[tokio::test]
async fn edit_between_discovery_and_capture_is_rejected() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use zf_flows::flow_format::SourceValidator;

    let f = Fixture::new();
    let file = f
        .flow(&flow(
            "selected",
            "set",
            json!({"field":"output","value":"done"}),
        ))
        .await;
    let path = file.path.clone();
    let source = file.source.clone().unwrap();
    let changed = AtomicBool::new(false);
    // A public validator callback provides a deterministic filesystem edit
    // immediately after discovery reads its bytes, without sleeps or races.
    let validator = move |doc: &Composition| -> anyhow::Result<()> {
        GraphValidator::new(&RuntimePrimitives).validate(doc)?;
        if !changed.swap(true, Ordering::SeqCst) {
            std::fs::write(&path, format!("{source}\n// external edit\n"))?;
        }
        Ok(())
    };
    let store = FlowStore::new(f._root.path().join("home"), Arc::new(validator));
    let mut request = selection(&file);
    // The capture barrier also applies when the caller has no selection pin.
    request.flow_hashes.clear();
    let error = prepare(&store, &f.workspace, &request).await.unwrap_err();
    assert!(
        error.to_string().contains("Flow changed while resolving"),
        "{error:#}"
    );
}
