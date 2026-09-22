use std::{collections::BTreeMap, path::Path, time::Duration};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tower::ServiceExt;
use zf_compiler::prepared::PreparedRuntime;
use zf_context::context::ContextBlock;
use zf_context::context::ContextCapability;
use zf_context::context::ContextExpr;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_context::context_source;
use zf_core::types::DataType;
use zf_flows::composition::BridgeDefinition;
use zf_flows::composition::Connection;
use zf_flows::composition::Endpoint;
use zf_flows::composition::InvocationKind;
use zf_flows::composition::RouteMode;
use zf_flows::schema::Composition;
use zf_storage::bridge_store::BridgeStore;
use zf_storage::content_store::ContentStore;
use zf_storage::context_store;
use zf_storage::flow_store::FlowStore;
use zf_storage::workspaces::Workspace;

async fn request(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "{path}: {}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}

fn flow(id: &str, parent: bool) -> Composition {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({
        "contract":{"entries":{"main":contract}},
        "entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},
        "interactive":true
    });
    let (action, continuation, output) = if parent {
        exports["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["tool"]}});
        exports["branches"] = json!({"work":"action"});
        let capability = ContextCapability::new(
            "delegate",
            DataType::Record {
                fields: BTreeMap::from([("input".into(), DataType::Text)]),
            },
            DataType::Text,
        );
        let strategy = ContextStrategy::new("delegate-context", "Delegate")
            .capability(capability.clone())
            .with_program(vec![ContextBlock::emit(
                "task",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!("Call the worker")),
            )]);
        let source = context_source::generate(&strategy).unwrap();
        (
            json!({"kind":"agent","label":"Delegate","config":{
                "provider":"fixture",
                "capabilityGrants":[capability],
                "contextProgram":{
                    "strategy":strategy,"source":source,
                    "hash":context_store::hash(source.as_bytes()),"types":{},"bindings":{}
                },
                "fixtureSteps":[{"tool":"delegate","args":{"input":"child input"}}]
            }}),
            json!({"kind":"tool","label":"Dispatch","config":{"tool":"execute_next_call"}}),
            json!({"text":"{{toolResults}}"}),
        )
    } else {
        (
            json!({"kind":"tool","label":"One effect","config":{
                "tool":"exec","arguments":{"command":"printf x >> visits"}
            }}),
            json!({"kind":"input","label":"Question","config":{
                "field":"output","prompt":"Continue?","responseType":"text"
            }}),
            json!({"inputField":"output"}),
        )
    };
    serde_json::from_value(json!({
        "formatVersion":3,"id":id,"name":id,
        "nodes":[
            {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":exports}}},
            {"id":"action","position":{"x":16,"y":0},"data":action},
            {"id":"continuation","position":{"x":32,"y":0},"data":continuation},
            {"id":"publish","position":{"x":48,"y":0},"data":{"kind":"output","label":"Publish","config":output}},
            {"id":"end","position":{"x":64,"y":0},"data":{"kind":"end","label":"End","config":{}}}
        ],
        "edges":[
            {"id":"a","source":"start","target":"action"},
            {"id":"b","source":"action","target":"continuation"},
            {"id":"c","source":"continuation","target":"publish"},
            {"id":"d","source":"publish","target":"end"}
        ]
    }))
    .unwrap()
}

async fn wait_for_boundary(app: &Router, id: &str, workspace: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let run = request(
                app,
                "GET",
                &format!("/api/runs/{id}?workspaceId={workspace}"),
                None,
            )
            .await;
            if run["status"] != "running" {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Run did not reach a durable boundary")
}

async fn content_store(data: &Path) -> ContentStore {
    ContentStore::from_pool(
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(data.join("zedflow.db"))
                    .read_only(true),
            )
            .await
            .unwrap(),
    )
}

async fn graph_references(store: &ContentStore, id: &str) -> BTreeMap<(String, String), String> {
    let mut references = BTreeMap::new();
    for record in store.records(id).await.unwrap() {
        if matches!(
            record.kind.as_str(),
            "route-inputs" | "route-visits" | "route-dispatches"
        ) {
            let value = store.resolve(&record.value_ref).await.unwrap();
            let reference = value["graphRef"]
                .as_str()
                .expect("Route record must pin its resolved graph");
            let graph: PreparedRuntime =
                serde_json::from_value(store.resolve(reference).await.unwrap()).unwrap();
            graph
                .validate(&zf_runtime::materialize::RuntimePrimitives)
                .unwrap();
            assert_eq!(graph.graph.routes.len(), 1);
            assert_eq!(graph.flows.len(), 2);
            references.insert((record.kind, record.key), reference.to_owned());
        }
    }
    for kind in ["route-inputs", "route-visits", "route-dispatches"] {
        assert!(
            references.keys().any(|(actual, _)| actual == kind),
            "Missing {kind} proof"
        );
    }
    references
}

#[tokio::test]
async fn imported_model_tool_route_resumes_its_waiting_receipt_without_repeating_child_effects() {
    let root = tempfile::tempdir().unwrap();
    let source_data = root.path().join("data");
    let source_path = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir(&source_path).unwrap();
    std::fs::create_dir(&home).unwrap();
    let app =
        zf_serve::server::router_with_home(source_data.clone(), source_path.clone(), vec![], {
            let home = home.clone();
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let workspaces = request(&app, "GET", "/api/workspaces", None).await;
    let workspace: Workspace = serde_json::from_value(workspaces[0].clone()).unwrap();
    let store = FlowStore::new(
        home,
        std::sync::Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        )),
    );
    let parent = store
        .store(&workspace, flow("parent", true), "workspace", None, None)
        .await
        .unwrap();
    let child = store
        .store(&workspace, flow("child", false), "workspace", None, None)
        .await
        .unwrap();
    let bridge = BridgeDefinition::new()
        .import("worker", &child.key)
        .connect(
            "work",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("worker", "main"),
                RouteMode::CallAwait,
                InvocationKind::Tool,
            )
            .tool("delegate"),
        );
    BridgeStore::new(workspace.path.clone())
        .unwrap()
        .save("integration", &bridge, None)
        .await
        .unwrap();
    let started = request(
        &app,
        "POST",
        "/api/runs",
        Some(json!({
            "workspaceId":workspace.id,
            "runtimeSelection":{"flow":parent.key,"entry":"main","bridges":["integration"]},
            "input":{"input":"question"}
        })),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    let waiting = wait_for_boundary(&app, id, &workspace.id).await;
    assert_eq!(waiting["status"], "waiting", "{waiting}");
    assert_eq!(waiting["wait"]["kind"], "input");
    assert_eq!(
        waiting["wait"]["nodePath"],
        "integration/worker/continuation"
    );
    assert_eq!(
        std::fs::read_to_string(source_path.join("visits")).unwrap(),
        "x"
    );

    let source_content = content_store(&source_data).await;
    let mut receipts = BTreeMap::new();
    for record in source_content
        .records_of_kind(id, "receipts")
        .await
        .unwrap()
    {
        let value = source_content.resolve(&record.value_ref).await.unwrap();
        receipts.insert(value["name"].as_str().unwrap().to_owned(), value);
    }
    assert_eq!(
        receipts.len(),
        2,
        "One model route and one child effect: {receipts:?}"
    );
    assert_eq!(receipts["exec"]["status"], "completed");
    assert_eq!(receipts["delegate"]["status"], "waiting");
    assert_eq!(
        receipts["delegate"]["result"]["__zedflowRoute"]["status"],
        "waiting"
    );
    let references = graph_references(&source_content, id).await;

    let exported = request(
        &app,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":workspace.id,"sessionIds":[id]})),
    )
    .await;
    let target_path = root.path().join("import-workspace");
    let target_data = root.path().join("import-data");
    std::fs::create_dir(&target_path).unwrap();
    let imported_app =
        zf_serve::server::router_with_home(target_data.clone(), target_path.clone(), vec![], {
            let home = root.path().join("import-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let workspaces = request(&imported_app, "GET", "/api/workspaces", None).await;
    let target_id = workspaces[0]["id"].as_str().unwrap();
    let imported = request(
        &imported_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":target_id,"path":exported["exports"][0]["path"]})),
    )
    .await;
    assert_eq!(imported["runs"][0]["status"], "waiting", "{imported}");
    assert_eq!(
        imported["runs"][0]["import"]["resumeBlocked"],
        json!([]),
        "{imported}"
    );
    assert!(!target_path.join("visits").exists());
    let imported_content = content_store(&target_data).await;
    assert_eq!(graph_references(&imported_content, id).await, references);

    request(
        &imported_app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={target_id}"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"imported continuation"})),
    )
    .await;
    let completed = wait_for_boundary(&imported_app, id, target_id).await;
    assert_eq!(completed["status"], "completed", "{completed}");
    assert_eq!(
        completed["state"]["toolResults"][0]["result"]["result"],
        "imported continuation"
    );
    assert!(
        !target_path.join("visits").exists(),
        "An imported checkpoint must not repeat the completed shell effect"
    );
    assert_eq!(
        std::fs::read_to_string(source_path.join("visits")).unwrap(),
        "x"
    );
    let receipts = imported_content
        .records_of_kind(id, "receipts")
        .await
        .unwrap();
    assert_eq!(
        receipts.len(),
        2,
        "Resume must retain the same two durable calls"
    );
    for receipt in receipts {
        let value = imported_content.resolve(&receipt.value_ref).await.unwrap();
        assert_eq!(value["status"], "completed", "{value}");
    }
}
