use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use zf_flows::composition::*;
use zf_flows::schema::Composition;
use zf_storage::bridge_store::BridgeStore;
use zf_storage::flow_store::FlowStore;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    )
}
fn flow(id: &str, root: bool) -> Composition {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut exports = json!({"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":false});
    if root {
        exports["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["node"]}});
        exports["branches"] = json!({"work":"action"});
    }
    let action = if root {
        json!({"kind":"route","label":"Call worker","config":{"branch":"work","inputField":"input","field":"output"}})
    } else {
        json!({"kind":"set","label":"Result","config":{"field":"output","value":"child result"}})
    };
    serde_json::from_value(json!({"formatVersion":2,"id":id,"name":id,"nodes":[
 {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":exports}}},
 {"id":"action","position":{"x":16,"y":0},"data":action},
 {"id":"publish","position":{"x":32,"y":0},"data":{"kind":"output","label":"Publish","config":{"inputField":"output"}}},
 {"id":"end","position":{"x":48,"y":0},"data":{"kind":"end","label":"End","config":{}}}
 ],"edges":[{"id":"a","source":"start","target":"action"},{"id":"b","source":"action","target":"publish"},{"id":"c","source":"publish","target":"end"}]})).unwrap()
}

async fn prepared_fixture(
    root: &std::path::Path,
) -> (
    Router,
    zf_storage::workspaces::Workspace,
    zf_compiler::prepared::PreparedRuntime,
    Value,
) {
    let workspace = root.join("workspace");
    let home = root.join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let app = zf_serve::server::router_with_home(root.join("data"), workspace, vec![], {
        let home = home.clone();
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let (_, workspaces) = request(&app, "GET", "/api/workspaces", None).await;
    let workspace: zf_storage::workspaces::Workspace =
        serde_json::from_value(workspaces[0].clone()).unwrap();
    let mut parent = flow("parent", true);
    parent.format_version = 3;
    parent.nodes[0].data.config["exports"]["interactive"] = json!(true);
    let mut inner = flow("inner", false);
    inner.format_version = 3;
    inner.nodes[0].data.config = json!({});
    inner.nodes[1].data.kind = "tool".into();
    inner.nodes[1].data.config =
        json!({"tool":"exec","arguments":{"command":"printf x >> visits"}});
    inner.nodes.push(serde_json::from_value(json!({"id":"question","position":{"x":24,"y":0},"data":{"kind":"input","label":"Question","config":{"field":"output","prompt":"Reply","responseType":"text"}}})).unwrap());
    inner
        .edges
        .iter_mut()
        .find(|e| e.source == "action")
        .unwrap()
        .target = "question".into();
    inner.edges.push(
        serde_json::from_value(json!({"id":"d","source":"question","target":"publish"})).unwrap(),
    );
    let mut child = flow("child", false);
    child.format_version = 3;
    child.nodes[0].data.config["exports"]["interactive"] = json!(true);
    child.nodes[1].data.kind = "subgraph".into();
    child.nodes[1].data.config = json!({"composition":inner});
    child.nodes[2].data.config = json!({"inputField":"output"});
    let store = FlowStore::new(
        home,
        std::sync::Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        )),
    );
    let parent = store
        .store(&workspace, parent, "workspace", None, None)
        .await
        .unwrap();
    let child = store
        .store(&workspace, child, "workspace", None, None)
        .await
        .unwrap();
    // Comments are part of the frozen source bytes; export must never regenerate.
    let child_path = std::path::Path::new(&child.path).join("flow.rs");
    let source = std::fs::read_to_string(&child_path).unwrap();
    std::fs::write(
        &child_path,
        format!("{source}\n// exact exported child marker\n"),
    )
    .unwrap();
    let bridge = BridgeDefinition::new()
        .import("worker", &child.key)
        .connect(
            "work",
            Connection::new(
                Endpoint::new("root", "work"),
                Endpoint::new("worker", "main"),
                RouteMode::CallAwait,
                InvocationKind::Node,
            ),
        );
    BridgeStore::new(workspace.path.clone())
        .unwrap()
        .save("integration", &bridge, None)
        .await
        .unwrap();
    let selection = json!({"flow":parent.key,"entry":"main","bridges":["integration"]});
    let (status, value) = request(
        &app,
        "POST",
        "/api/runtime-graphs/prepare",
        Some(json!({"workspaceId":workspace.id,"selection":selection})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    (
        app,
        workspace,
        serde_json::from_value(value["runtime"].clone()).unwrap(),
        selection,
    )
}
#[tokio::test]
async fn export_contains_exact_flow_and_bridge_modules_and_rejects_missing_pins() {
    let root = tempfile::tempdir().unwrap();
    let (_, _, mut prepared, _) = prepared_fixture(root.path()).await;
    let exported = zf_compiler::compiler::compile_prepared(
        prepared.clone(),
        &zf_runtime::materialize::RuntimePrimitives,
    )
    .map_err(|errors| anyhow::anyhow!("{errors:?}"))
    .and_then(|plan| {
        zf_compiler::export::export_runtime(&plan, &zf_runtime::runtime_export::support())
    })
    .unwrap();
    let files = &exported.files;
    for (index, (_, flow)) in prepared.flows.iter().enumerate() {
        assert_eq!(
            files[&format!("flows/instance-{index}/flow.rs")],
            flow.source.as_bytes()
        );
    }
    for (index, (_, source)) in prepared.definitions.bridge_sources.iter().enumerate() {
        assert_eq!(
            files[&format!("runner/src/bridges/bridge_{index}.rs")],
            source.as_bytes()
        );
    }
    // Export retains workspace version constraints; Cargo.lock freezes every
    // external package (including checksums), not only ADK's exact requirements.
    let exported_lock = std::str::from_utf8(&files["Cargo.lock"]).unwrap();
    let support = zf_runtime::runtime_export::support();
    let support_lock = std::str::from_utf8(&support.files["Cargo.lock"]).unwrap();
    let external_packages = |lock: &str| -> std::collections::BTreeSet<String> {
        lock.split("[[package]]")
            .filter(|block| block.contains("source = "))
            .map(|block| block.trim().to_owned())
            .collect()
    };
    assert!(!external_packages(support_lock).is_empty());
    assert_eq!(
        external_packages(exported_lock),
        external_packages(support_lock)
    );
    prepared.definitions.bridge_sources.clear();
    prepared.definitions.bridge_hashes.clear();
    assert!(
        zf_compiler::compiler::compile_prepared(
            prepared.clone(),
            &zf_runtime::materialize::RuntimePrimitives
        )
        .map_err(|errors| anyhow::anyhow!("{errors:?}"))
        .and_then(|plan| zf_compiler::export::export_runtime(
            &plan,
            &zf_runtime::runtime_export::support()
        ))
        .is_err()
    );
}
async fn wait_for(app: &Router, id: &str, workspace: &str) -> Value {
    for _ in 0..500 {
        let (_, run) = request(
            app,
            "GET",
            &format!("/api/runs/{id}?workspaceId={workspace}"),
            None,
        )
        .await;
        if run["status"] != "running" {
            return run;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("Run did not reach a boundary")
}
async fn cargo_run(
    project: &std::path::Path,
    workspace: &std::path::Path,
    data: &std::path::Path,
    input: &str,
) -> Value {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(300),
        tokio::process::Command::new("cargo")
            .args(["run", "--offline", "--quiet", "--manifest-path"])
            .arg(project.join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(workspace)
            .arg("--home")
            .arg(workspace.join(".fixture-home"))
            .arg("--data")
            .arg(data)
            .args(["--run-id", "parity", "--input", input])
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .unwrap_or_else(|| "/tmp/zedflow-adk-target".into()),
            )
            .output(),
    )
    .await
    .expect("exported runtime timed out")
    .unwrap();
    assert!(
        output.status.success(),
        "export execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stdout)))
}
#[tokio::test]
async fn exact_compiled_sources_match_daemon_with_nested_wait_and_receipt_resume() {
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let (app, workspace, prepared, selection) = prepared_fixture(root.path()).await;
    let exported = zf_compiler::compiler::compile_prepared(
        prepared.clone(),
        &zf_runtime::materialize::RuntimePrimitives,
    )
    .map_err(|errors| anyhow::anyhow!("{errors:?}"))
    .and_then(|plan| {
        zf_compiler::export::export_runtime(&plan, &zf_runtime::runtime_export::support())
    })
    .unwrap();
    let project = root.path().join("cargo");
    for (relative, content) in &exported.files {
        let path = project.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
    let portable = root.path().join("portable");
    std::fs::create_dir(&portable).unwrap();
    let data = root.path().join("portable-data");
    let first = cargo_run(&project, &portable, &data, r#"{"input":"question"}"#).await;
    assert_eq!(first["status"], "waiting", "{first}");
    assert!(
        first["interrupt"]
            .to_string()
            .contains("integration/worker/action/question"),
        "{first}"
    );
    assert_eq!(
        std::fs::read_to_string(portable.join("visits")).unwrap(),
        "x"
    );
    let second = cargo_run(
        &project,
        &portable,
        &data,
        r#"{"answer:integration/worker/action/question":"continued"}"#,
    )
    .await;
    assert_eq!(second["status"], "completed", "{second}");
    assert_eq!(second["state"]["response"], "continued");
    assert_eq!(
        std::fs::read_to_string(portable.join("visits")).unwrap(),
        "x",
        "resuming native nested sources repeated a committed effect"
    );
    let (status,started)=request(&app,"POST","/api/runs",Some(json!({"workspaceId":workspace.id,"runtimeSelection":selection,"input":{"input":"question"}}))).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let id = started["id"].as_str().unwrap();
    let waiting = wait_for(&app, id, &workspace.id).await;
    assert_eq!(waiting["status"], "waiting", "{waiting}");
    assert_eq!(
        waiting["wait"]["nodePath"],
        "integration/worker/action/question"
    );
    let (status, ack) = request(
        &app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={}", workspace.id),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"continued"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ack}");
    let completed = wait_for(&app, id, &workspace.id).await;
    assert_eq!(completed["status"], "completed", "{completed}");
    assert_eq!(completed["state"]["response"], second["state"]["response"]);
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("visits")).unwrap(),
        "x"
    );
    let events = std::fs::read_to_string(data.join("parity/events.jsonl")).unwrap();
    assert!(events.contains("tool_result"));
    assert!(events.contains("receiptRef"));
    assert!(events.contains("route_status"));
}
