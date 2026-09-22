use adk_graph::{ExecutionConfig, NodeContext, State};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{collections::BTreeMap, sync::Arc};
use tempfile::TempDir;
use zf_storage::content_store::ContentStore;

use zf_compiler::prepared::FrozenFlow;
use zf_compiler::prepared::PreparedRuntime;
use zf_context::context_source;
use zf_context::starters;
use zf_execution::route_runtime::RouteRuntime;
use zf_flows::composition::CompositionCatalog;
use zf_flows::composition::ResolveRequest;
use zf_flows::flow_contract;
use zf_flows::schema::Composition;
use zf_runtime::operations;
use zf_runtime::runtime::RunServices;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::context_store;
use zf_storage::data::DataRegistry;

const CHILD_RESULT: &str = "Résultat documentaire produit par le flow fixture";

fn route_config() -> Value {
    json!({"branch":"work","invocation":"condition","inputField":"input","field":"routeResult","fallback":""})
}

fn services(root: &std::path::Path) -> Arc<RunServices> {
    RunServices::new(
        "harness-routing".into(),
        root.into(),
        root.join("data"),
        ContextSnapshot {
            cwd: root.into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap()
}

#[tokio::test]
async fn unconnected_conditional_plug_continues_and_clears_a_previous_result() {
    let root = tempfile::tempdir().unwrap();
    let initial = State::from([("routeResult".into(), json!(CHILD_RESULT))]);
    let output = operations::execute_with_services(
        "route",
        &route_config(),
        NodeContext::new(initial.clone(), ExecutionConfig::new("unconnected"), 0),
        "routing",
        services(root.path()),
    )
    .await
    .unwrap();
    assert!(output.interrupt.is_none());
    assert_eq!(output.updates["routeResult"], "");
    let mut reconstructed = initial;
    reconstructed.extend(output.updates);
    assert_eq!(reconstructed["routeResult"], "");
}

#[tokio::test]
async fn unconnected_required_or_explicitly_selected_routes_fail() {
    let root = tempfile::tempdir().unwrap();
    let services = services(root.path());
    for config in [
        json!({"branch":"work","invocation":"node","fallback":""}),
        json!({"branch":"work","invocation":"condition","routeId":"bridge/work","fallback":""}),
    ] {
        let result = operations::execute_with_services(
            "route",
            &config,
            NodeContext::new(State::new(), ExecutionConfig::new("required"), 0),
            "routing",
            services.clone(),
        )
        .await;
        assert!(
            result
                .err()
                .expect("Required or selected routing must fail without a host")
                .to_string()
                .contains("Runtime Graph résolu"),
            "An explicit selection or mandatory route must not silently fall through"
        );
    }
}

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}

fn edge(source: &str, target: &str) -> Value {
    json!({"id":format!("{source}-{target}"),"source":source,"target":target})
}

fn root_flow() -> Composition {
    let strategy = starters::harness_strategy();
    let source = context_source::generate(&strategy).unwrap();
    assert_eq!(context_source::parse(&source).unwrap(), strategy);
    let mut bindings = starters::bindings();
    bindings["routeResult"] = json!({"kind":"state","field":"routeResult"});
    let context = json!({
        "modelNode":"model","fanIn":"any",
        "contextProgram":{"strategy":strategy,"hash":context_store::hash(source.as_bytes()),"source":source,"bindings":bindings},
        "attachments":{"tools":{"items":[{"id":"read","name":"read"},{"id":"write","name":"write"},{"id":"edit","name":"edit"},{"id":"exec","name":"exec"}]}}
    });
    serde_json::from_value(json!({
        "formatVersion":3,"id":"harness-fixture","name":"Harness routing fixture",
        "channels":[{"name":"routeResult","reducer":"overwrite","default":""},{"name":"needsResearch","reducer":"overwrite","default":false}],
        "nodes":[
            node("start","start",json!({"exports":{
                "contract":{"entries":{"main":{"input":{"kind":"text"}}},"branches":{"work":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["condition"]}}},
                "entries":{"main":{"node":"start","inputField":"input"}},"branches":{"work":"routing"}
            }})),
            node("routing","route",route_config()),
            node("context","context",context),
            node("model","model",json!({"contextNode":"context","provider":"fixture","fixtureSteps":[{"echoRequest":true}],"field":"output","historyField":"messages","inputField":"input"})),
            node("end","end",json!({}))
        ],
        "edges":[edge("start","routing"),edge("routing","context"),edge("context","model"),edge("model","end")]
    }))
    .unwrap()
}

fn worker_flow() -> Composition {
    serde_json::from_value(json!({
        "formatVersion":3,"id":"worker-fixture","name":"Documentary fixture",
        "nodes":[
            node("start","start",json!({"exports":{
                "contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},
                "entries":{"main":{"node":"start","inputField":"input","outputField":"output"}}
            }})),
            node("result","set",json!({"field":"output","value":CHILD_RESULT})),
            node("end","end",json!({}))
        ],
        "edges":[edge("start","result"),edge("result","end")]
    }))
    .unwrap()
}

fn freeze(composition: Composition) -> FrozenFlow {
    let source = zf_flows::flow_source::render(
        &composition,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(
            zf_flows::flow_source::parse(
                &source,
                &zf_compiler::graph_compiler::GraphValidator::new(
                    &zf_runtime::materialize::RuntimePrimitives
                )
            )
            .unwrap()
        )
        .unwrap(),
        serde_json::to_value(&composition).unwrap()
    );
    FrozenFlow {
        key: composition.id.clone(),
        hash: zf_storage::flow_store::hash(source.as_bytes()),
        source,
        exports: flow_contract::validate(&composition).unwrap().unwrap(),
        composition,
    }
}

struct Fixture {
    _root: TempDir,
    services: Arc<RunServices>,
    store: ContentStore,
    prepared: PreparedRuntime,
}

impl Fixture {
    async fn new(ambiguous: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let root_flow = root_flow();
        let worker = worker_flow();
        let connection = json!({
            "from":{"instance":"root","port":"work"},"to":{"instance":"worker","port":"main"},
            "invocation":"condition","mode":"callAwait",
            "condition":{"kind":"compare","field":"needsResearch","operator":"eq","value":true}
        });
        let mut connections = json!({"research":connection});
        if ambiguous {
            connections["duplicate"] = connection;
        }
        let catalog: CompositionCatalog = serde_json::from_value(json!({
            "flows":{"harness-fixture":flow_contract::read(&root_flow).unwrap().unwrap().contract,"worker-fixture":flow_contract::read(&worker).unwrap().unwrap().contract},
            "bridges":{"bridge":{"imports":{"worker":{"flow":"worker-fixture"}},"connections":connections}}
        }))
        .unwrap();
        let prepared = PreparedRuntime {
            graph: zf_compiler::resolve::resolve(
                &catalog,
                &ResolveRequest {
                    flow: "harness-fixture".into(),
                    entry: "main".into(),
                    bridges: vec!["bridge".into()],
                },
            )
            .unwrap(),
            flows: BTreeMap::from([
                ("root".into(), freeze(root_flow)),
                ("bridge/worker".into(), freeze(worker)),
            ]),
            definitions: Default::default(),
        };
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(root.path().join("store.sqlite"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        let store = ContentStore::new(pool.clone()).await.unwrap();
        let services = services(root.path());
        services.set_content_store(store.clone());
        services
            .set_data_registry(
                DataRegistry::new(pool, store.clone(), &services.id)
                    .await
                    .unwrap(),
            )
            .unwrap();
        let checkpointer = Arc::new(StoredCheckpointer::new(
            zf_storage::contracts::CheckpointStore::new(store.clone())
                .await
                .unwrap(),
        ));
        let runtime = RouteRuntime::new(prepared.clone(), &services, checkpointer, None).unwrap();
        services.set_dynamic_capabilities(runtime.clone());
        runtime.initialize(&State::new()).await.unwrap();
        Self {
            _root: root,
            services,
            store,
            prepared,
        }
    }

    async fn invoke(&self, id: &str, guard: bool, previous_result: &str) -> State {
        let graph = zf_runtime::materialize::build_with_services(
            &self.prepared.flows["root"].composition,
            self.services.clone(),
            None,
            None,
        )
        .unwrap();
        graph
            .invoke(
                State::from([
                    ("input".into(), json!("Question documentaire")),
                    ("needsResearch".into(), json!(guard)),
                    ("routeResult".into(), json!(previous_result)),
                ]),
                ExecutionConfig::new(id),
            )
            .await
            .unwrap()
    }

    async fn visits(&self) -> usize {
        self.store
            .records(&self.services.id)
            .await
            .unwrap()
            .iter()
            .filter(|record| record.kind == "route-visits")
            .count()
    }
}

#[tokio::test]
async fn guarded_child_result_reaches_context_and_next_skipped_pass_drops_it() {
    let fixture = Fixture::new(false).await;
    for (index, guard) in [false, true, false].into_iter().enumerate() {
        let state = fixture
            .invoke(&format!("pass-{index}"), guard, CHILD_RESULT)
            .await;
        assert_eq!(state["routeResult"], if guard { CHILD_RESULT } else { "" });
        let request: Value = serde_json::from_str(state["output"].as_str().unwrap()).unwrap();
        let text: Vec<_> = request["contents"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|content| content["parts"].as_array().unwrap())
            .filter_map(|part| part["text"].as_str())
            .collect();
        assert_eq!(text.contains(&CHILD_RESULT), guard, "{request:#}");
        assert!(text.contains(&"Question documentaire"));
        assert_eq!(request["tools"].as_object().unwrap().len(), 4);
        assert_eq!(fixture.visits().await, usize::from(index > 0));
    }
}

#[tokio::test]
async fn ambiguous_conditional_routes_fail_before_any_child_visit() {
    let fixture = Fixture::new(true).await;
    let result = operations::execute_with_services(
        "route",
        &route_config(),
        NodeContext::new(
            State::from([
                ("input".into(), json!("Question documentaire")),
                ("needsResearch".into(), json!(true)),
            ]),
            ExecutionConfig::new("ambiguous"),
            0,
        ),
        "root/routing",
        fixture.services.clone(),
    )
    .await;
    let error = result
        .err()
        .expect("Ambiguous routes must fail")
        .to_string();
    assert!(
        error.contains("Expected one eligible route, found 2"),
        "{error}"
    );
    assert_eq!(fixture.visits().await, 0);
}

#[tokio::test]
async fn exact_exported_harness_keeps_unconnected_route_fallback_and_model_request() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let doc = root_flow();
    let source = zf_flows::flow_source::render(
        &doc,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let export = zf_compiler::export::export_single(
        &doc,
        &zf_flows::flow_source::render(
            &doc,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )
        .unwrap(),
        None,
        &zf_runtime::materialize::RuntimePrimitives,
        &zf_runtime::runtime_export::support(),
    )
    .unwrap();
    let package = tempfile::tempdir().unwrap();
    let mut contains_source = false;
    for (relative, content) in &export.files {
        contains_source |= content.as_slice() == source.as_bytes();
        let path = package.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    assert!(
        contains_source,
        "Cargo export must contain the exact authored source"
    );
    let workspace = tempfile::tempdir().unwrap();
    let input = json!({"input":"Question documentaire","routeResult":CHILD_RESULT});
    let expected =
        zf_runtime::materialize::build_with_services(&doc, services(workspace.path()), None, None)
            .unwrap()
            .invoke(
                serde_json::from_value(input.clone()).unwrap(),
                ExecutionConfig::new("daemon-fallback"),
            )
            .await
            .unwrap();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["run", "--quiet", "--offline", "--manifest-path"])
            .arg(package.path().join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(workspace.path())
            .arg("--home")
            .arg(workspace.path().join(".fixture-home"))
            .arg("--data")
            .arg(workspace.path().join("export-data"))
            .args(["--run-id", "exported-fallback", "--input"])
            .arg(input.to_string())
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .unwrap_or_else(|| "/tmp/zedflow-adk-target".into()),
            )
            .output(),
    )
    .await
    .expect("Cargo fixture did not finish within five minutes")
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(state["status"], "completed", "{state}");
    let state = &state["state"];
    let actual_request: Value = serde_json::from_str(state["output"].as_str().unwrap()).unwrap();
    let expected_request: Value =
        serde_json::from_str(expected["output"].as_str().unwrap()).unwrap();
    assert_eq!(actual_request, expected_request);
    assert_eq!(state["routeResult"], "");
    assert!(!actual_request.to_string().contains(CHILD_RESULT));
}
