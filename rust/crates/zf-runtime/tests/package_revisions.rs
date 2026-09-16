use adk_graph::{ExecutionConfig, Node, NodeContext, NodeOutput, State};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use zf_flows::{
    package::{PackageNode, PackageSnapshot},
    schema::Composition,
};
use zf_runtime::{
    materialize::RuntimePrimitives,
    revisions::{self, RevisionDefinition, RevisionRuntime},
};
use zf_storage::content_store::ContentStore;

fn definition(asset: &str) -> RevisionDefinition {
    let composition: Composition = serde_json::from_value(json!({"id":"fixture","name":"Fixture","formatVersion":2,"nodes":[
        {"id":"s","position":{"x":0,"y":0},"data":{"label":"start","kind":"start","config":{"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"s","inputField":"input","outputField":"output"}},"interactive":false}}}},
        {"id":"work","position":{"x":0,"y":0},"data":{"label":"work","kind":"set","config":{"field":"output","value":"same"}}},
        {"id":"e","position":{"x":0,"y":0},"data":{"label":"end","kind":"end","config":{}}}
    ],"edges":[{"id":"a","source":"s","target":"work"},{"id":"b","source":"work","target":"e"}]})).unwrap();
    let source = zf_flows::flow_format::render(
        &composition,
        &zf_compiler::graph_compiler::GraphValidator::new(&RuntimePrimitives),
    )
    .unwrap();
    let node = PackageNode {
        manifest_source: json!({"formatVersion":1,"id":"fixture","name":"Fixture","entry":"flow.rs","files":["flow.rs","asset.txt"]}).to_string(),
        files: BTreeMap::from([("flow.rs".into(), source.as_bytes().to_vec()), ("asset.txt".into(), asset.as_bytes().to_vec())]),
        dependencies: BTreeMap::new(),
    };
    let root = node.revision();
    RevisionDefinition {
        key: "fixture".into(),
        hash: zf_storage::flow_store::hash(source.as_bytes()),
        source,
        composition,
        package: Some(PackageSnapshot {
            root: root.clone(),
            packages: BTreeMap::from([(root, node)]),
        }),
        context_selections: BTreeMap::new(),
    }
}
struct Probe {
    pins: Arc<Mutex<Vec<(bool, Value)>>>,
    initial: bool,
}
#[async_trait::async_trait]
impl Node for Probe {
    fn name(&self) -> &str {
        "work"
    }
    async fn execute(&self, _: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
        self.pins
            .lock()
            .unwrap()
            .push((self.initial, revisions::current_revision().unwrap()));
        Ok(NodeOutput::new())
    }
}
fn wrapped(runtime: &Arc<RevisionRuntime>, pins: &Arc<Mutex<Vec<(bool, Value)>>>) -> Arc<dyn Node> {
    let factory_pins = pins.clone();
    runtime.wrap(
        "",
        "work",
        Arc::new(Probe {
            pins: pins.clone(),
            initial: true,
        }),
        Arc::new(move |_, _| {
            Ok(Arc::new(Probe {
                pins: factory_pins.clone(),
                initial: false,
            }))
        }),
    )
}
fn context(thread: &str, step: usize, pending: bool) -> NodeContext {
    let state = if pending {
        State::from([("toolCalls".into(), json!([{"id":"pending"}]))])
    } else {
        State::new()
    };
    NodeContext::new(state, ExecutionConfig::new(thread), step)
}
#[tokio::test]
async fn asset_revision_keeps_exact_passages_and_restart_pins_without_source_collision() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("package.db");
    let open = || {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
    };
    let store = ContentStore::new(open().await.unwrap()).await.unwrap();
    let old = definition("old");
    let new = definition("new");
    assert_eq!(old.hash, new.hash);
    assert_ne!(old.revision(), new.revision());
    let runtime = RevisionRuntime::new(
        store.clone(),
        "run",
        BTreeMap::from([("".into(), old.clone())]),
    )
    .await
    .unwrap();
    let pins = Arc::new(Mutex::new(Vec::new()));
    let node = wrapped(&runtime, &pins);
    node.execute(&context("thread", 0, false)).await.unwrap();
    node.execute(&context("pending-thread", 0, false))
        .await
        .unwrap();
    runtime.publish("", new.clone()).await.unwrap();
    node.execute(&context("thread", 0, false)).await.unwrap();
    node.execute(&context("pending-thread", 1, true))
        .await
        .unwrap();
    node.execute(&context("thread", 1, false)).await.unwrap();
    let entries = pins.lock().unwrap().clone();
    for (initial, pin) in &entries[..4] {
        assert!(*initial);
        assert_eq!(pin["definitionRevision"], old.revision());
    }
    assert!(
        !entries[4].0,
        "changed package must not reuse the initial native node"
    );
    assert_eq!(entries[4].1["definitionRevision"], new.revision());
    assert_eq!(entries[3].1["diagnostic"]["code"], "pending_tool_calls");
    assert_eq!(
        store
            .records_of_kind("run", "revision-definitions")
            .await
            .unwrap()
            .len(),
        2
    );
    let selected = revisions::checkpoint_definition(&store, "run", "thread", 1, "")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.package, new.package);
    let run = json!({"id":"run","flowRef":{"key":"fixture"},"composition":old.composition,"flowSource":old.source,"flowPackage":old.package,
        "activities":[{"path":"work","occurrenceId":"old","flowRevision":entries[0].1},{"path":"work","occurrenceId":"new","flowRevision":entries[4].1}]});
    for (occurrence, expected) in [("old", &old), ("new", &new)] {
        let value = zf_runtime::inspection::definition(
            &store,
            &run,
            zf_runtime::inspection::DefinitionQuery {
                occurrence_id: Some(occurrence.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(value["definitionRevision"], expected.revision());
        assert_eq!(value["package"], json!(expected.package));
    }
    drop(node);
    drop(runtime);
    drop(store);
    let reopened = ContentStore::new(open().await.unwrap()).await.unwrap();
    let runtime = RevisionRuntime::new(reopened.clone(), "run", BTreeMap::from([("".into(), old)]))
        .await
        .unwrap();
    wrapped(&runtime, &pins)
        .execute(&context("thread", 1, false))
        .await
        .unwrap();
    assert_eq!(
        pins.lock().unwrap().last().unwrap().1["definitionRevision"],
        new.revision()
    );
    assert_eq!(
        revisions::checkpoint_definition(&reopened, "run", "thread", 1, "")
            .await
            .unwrap()
            .unwrap()
            .package,
        new.package
    );
}
#[tokio::test]
async fn corrupt_package_and_unrelated_authored_source_are_rejected_before_publication() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let base = definition("base");
    let runtime = RevisionRuntime::new(
        store.clone(),
        "run",
        BTreeMap::from([("root".into(), base.clone())]),
    )
    .await
    .unwrap();
    let mut corrupt = definition("next");
    let package = corrupt.package.as_mut().unwrap();
    package
        .packages
        .get_mut(&package.root)
        .unwrap()
        .files
        .insert("asset.txt".into(), b"tampered".to_vec());
    assert!(runtime.publish("root", corrupt).await.is_err());
    let mut unrelated = base.clone();
    unrelated.composition.nodes[1].data.config["value"] = json!("unrelated");
    unrelated.source = zf_flows::flow_format::render(
        &unrelated.composition,
        &zf_compiler::graph_compiler::GraphValidator::new(&RuntimePrimitives),
    )
    .unwrap();
    unrelated.hash = zf_storage::flow_store::hash(unrelated.source.as_bytes());
    assert!(runtime.publish("root", unrelated).await.is_err());
    assert_eq!(
        store
            .records_of_kind("run", "revision-definitions")
            .await
            .unwrap()
            .len(),
        1
    );
    let mut legacy = base;
    legacy.package = None;
    assert_eq!(legacy.revision(), legacy.hash);
    assert!(
        serde_json::to_value(legacy)
            .unwrap()
            .get("package")
            .is_none()
    );
}

#[tokio::test]
async fn published_heads_advance_package_graph_pins_and_preserve_old_graph_snapshots() {
    use zf_compiler::{
        prepared::{CompilationSnapshot, prepare},
        programs::SourceSnapshot,
    };
    let old = definition("old");
    let new = definition("new");
    let plan = prepare(
        &CompilationSnapshot {
            flows: BTreeMap::from([(
                "fixture".into(),
                SourceSnapshot::capture(old.source.clone()),
            )]),
            packages: BTreeMap::from([("fixture".into(), old.package.clone().unwrap())]),
            ..Default::default()
        },
        &zf_flows::composition::ResolveRequest {
            flow: "fixture".into(),
            entry: "main".into(),
            bridges: vec![],
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        &RuntimePrimitives,
    )
    .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let instance = plan.graph.entry.instance.clone();
    let runtime = RevisionRuntime::new(
        store.clone(),
        "graph-run",
        BTreeMap::from([(instance.clone(), old.clone())]),
    )
    .await
    .unwrap();
    revisions::publish_mixed_unique(
        &store,
        &uuid::Uuid::new_v4().to_string(),
        &[],
        &[revisions::RuntimeGraphPublication {
            run_id: "graph-run".into(),
            baseline: plan.clone(),
            prepared: plan.clone(),
        }],
    )
    .await
    .unwrap();
    let pins = Arc::new(Mutex::new(Vec::new()));
    let node = runtime.wrap(
        &instance,
        "work",
        Arc::new(Probe {
            pins: pins.clone(),
            initial: true,
        }),
        {
            let pins = pins.clone();
            Arc::new(move |_, _| {
                Ok(Arc::new(Probe {
                    pins: pins.clone(),
                    initial: false,
                }))
            })
        },
    );
    node.execute(&context("graph-thread", 0, false))
        .await
        .unwrap();
    runtime.publish(&instance, new.clone()).await.unwrap();
    let current = revisions::latest_runtime_graph(&store, "graph-run")
        .await
        .unwrap()
        .unwrap();
    current.validate(&RuntimePrimitives).unwrap();
    assert_eq!(
        current.definitions.flow_packages["fixture"],
        new.package.clone().unwrap()
    );
    assert_eq!(current.definitions.flow_hashes["fixture"], new.hash);
    node.execute(&context("graph-thread", 1, false))
        .await
        .unwrap();
    let pins = pins.lock().unwrap().clone();
    let old_graph: zf_compiler::prepared_model::PreparedRuntime = serde_json::from_value(
        store
            .resolve(pins[0].1["graphRef"].as_str().unwrap())
            .await
            .unwrap(),
    )
    .unwrap();
    old_graph.validate(&RuntimePrimitives).unwrap();
    assert_eq!(
        old_graph.definitions.flow_packages["fixture"],
        old.package.clone().unwrap()
    );
    assert_ne!(pins[0].1["graphRef"], pins[1].1["graphRef"]);
    assert_eq!(pins[1].1["definitionRevision"], new.revision());
    let run = json!({"id":"graph-run","runtimeGraph":plan,"activities":[{"path":format!("{instance}/work"),"occurrenceId":"old","flowRevision":pins[0].1}]});
    let inspected = zf_runtime::inspection::definition(
        &store,
        &run,
        zf_runtime::inspection::DefinitionQuery {
            occurrence_id: Some("old".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(inspected["package"], json!(old.package));
    assert!(inspected["runtime"].is_object());
}

#[tokio::test]
async fn inspection_preserves_graph_catalogue_when_one_of_two_instances_keeps_old_package() {
    use zf_compiler::{
        prepared::{CompilationSnapshot, prepare},
        programs::SourceSnapshot,
    };
    use zf_flows::composition::{
        BridgeDefinition, Connection, Endpoint, InvocationKind, ResolveRequest, RouteMode,
    };
    let old = definition("old");
    let mut new = definition("new");
    new.composition.nodes[1].data.config["value"] = json!("changed source");
    new.source = zf_flows::flow_format::render(
        &new.composition,
        &zf_compiler::graph_compiler::GraphValidator::new(&RuntimePrimitives),
    )
    .unwrap();
    new.hash = zf_storage::flow_store::hash(new.source.as_bytes());
    let package = new.package.as_mut().unwrap();
    let mut node = package.packages.remove(&package.root).unwrap();
    node.files
        .insert("flow.rs".into(), new.source.as_bytes().to_vec());
    package.root = node.revision();
    package.packages.insert(package.root.clone(), node);
    let mut pilot = old.composition.clone();
    pilot.id = "pilot".into();
    pilot.nodes[0].data.config["exports"]["contract"]["branches"] = json!({"work":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["node"]}});
    pilot.nodes[0].data.config["exports"]["branches"] = json!({"work":"work"});
    pilot.nodes[1].data.kind = "route".into();
    pilot.nodes[1].data.config = json!({"branch":"work","inputField":"input","field":"output"});
    let bridge = BridgeDefinition::new()
        .import("first", "fixture")
        .import("second", "fixture")
        .connect(
            "first",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("first", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        )
        .connect(
            "second",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("second", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        );
    let snapshot = CompilationSnapshot {
        flows: BTreeMap::from([
            (
                "fixture".into(),
                SourceSnapshot::capture(old.source.clone()),
            ),
            (
                "pilot".into(),
                SourceSnapshot::capture(
                    zf_flows::flow_format::render(
                        &pilot,
                        &zf_compiler::graph_compiler::GraphValidator::new(&RuntimePrimitives),
                    )
                    .unwrap(),
                ),
            ),
        ]),
        packages: BTreeMap::from([("fixture".into(), old.package.clone().unwrap())]),
        bridges: BTreeMap::from([(
            "work".into(),
            SourceSnapshot::capture(zf_flows::bridge_source::generate(&bridge).unwrap()),
        )]),
        ..Default::default()
    };
    let plan = prepare(
        &snapshot,
        &ResolveRequest {
            flow: "pilot".into(),
            entry: "main".into(),
            bridges: vec!["work".into()],
        },
        &BTreeMap::new(),
        &BTreeMap::new(),
        &RuntimePrimitives,
    )
    .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let runtime = RevisionRuntime::new(
        store.clone(),
        "mixed-run",
        plan.flows
            .keys()
            .map(|instance| {
                (
                    instance.clone(),
                    RevisionDefinition::from_prepared(&plan, instance).unwrap(),
                )
            })
            .collect(),
    )
    .await
    .unwrap();
    revisions::publish_mixed_unique(
        &store,
        &uuid::Uuid::new_v4().to_string(),
        &[],
        &[revisions::RuntimeGraphPublication {
            run_id: "mixed-run".into(),
            baseline: plan.clone(),
            prepared: plan.clone(),
        }],
    )
    .await
    .unwrap();
    let instance = "work/first";
    let pins = Arc::new(Mutex::new(Vec::new()));
    let node = runtime.wrap(
        instance,
        "work",
        Arc::new(Probe {
            pins: pins.clone(),
            initial: true,
        }),
        {
            let pins = pins.clone();
            Arc::new(move |_, _| {
                Ok(Arc::new(Probe {
                    pins: pins.clone(),
                    initial: false,
                }))
            })
        },
    );
    node.execute(&context("mixed-thread", 0, false))
        .await
        .unwrap();
    revisions::publish_batch(
        &store,
        &["work/first", "work/second"].map(|instance| revisions::RevisionPublication {
            run_id: "mixed-run".into(),
            instance: instance.into(),
            baseline: old.composition.clone(),
            definition: new.clone(),
        }),
    )
    .await
    .unwrap();
    let latest = revisions::latest_runtime_graph(&store, "mixed-run")
        .await
        .unwrap()
        .unwrap();
    latest.validate(&RuntimePrimitives).unwrap();
    assert_eq!(latest.definitions.flow_hashes["fixture"], new.hash);
    assert_eq!(
        latest.definitions.flow_packages["fixture"],
        new.package.unwrap()
    );
    node.execute(&context("mixed-thread", 1, true))
        .await
        .unwrap();
    let pin = pins.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(pin["definitionRevision"], old.revision());
    let run = json!({"id":"mixed-run","runtimeGraph":plan,"activities":[{"path":format!("{instance}/work"),"occurrenceId":"pending","flowRevision":pin}]});
    let inspected = zf_runtime::inspection::definition(
        &store,
        &run,
        zf_runtime::inspection::DefinitionQuery {
            occurrence_id: Some("pending".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(inspected["definitionMatchesGraph"], false);
    assert_eq!(inspected["package"], json!(old.package));
    assert_eq!(
        inspected["runtime"]["instances"]["work/second"]["hash"],
        new.hash
    );
}
