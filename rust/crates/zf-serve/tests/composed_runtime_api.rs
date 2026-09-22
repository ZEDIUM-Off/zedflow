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
#[tokio::test]
async fn saved_flows_and_bridge_prepare_and_execute_native_adk_through_the_public_api() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir(&home).unwrap();
    let app =
        zf_serve::server::router_with_home(root.path().join("data"), workspace.clone(), vec![], {
            let home = home.clone();
            std::fs::create_dir_all(&home).unwrap();
            home
        })
        .await
        .unwrap();
    let (_, workspaces) = request(&app, "GET", "/api/workspaces", None).await;
    let workspace: zf_storage::workspaces::Workspace =
        serde_json::from_value(workspaces[0].clone()).unwrap();
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
    let mut donor = flow("unselected-types", false);
    donor.nodes[0].data.config["exports"]["types"] = json!({"SharedText":{"kind":"text"}});
    store
        .store(&workspace, donor, "workspace", None, None)
        .await
        .unwrap();
    let (status, listed) = request(
        &app,
        "GET",
        &format!("/api/flows?workspaceId={}", workspace.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed.as_array().unwrap().len(), 3);
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .all(|file| file.get("source").is_none() && file.get("package").is_none())
    );
    let (status, loaded) = request(
        &app,
        "GET",
        &format!("/api/flows/{}?workspaceId={}", parent.key, workspace.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(loaded["source"], parent.source.as_ref().unwrap().as_str());
    assert_eq!(
        loaded["package"],
        serde_json::to_value(&parent.package).unwrap()
    );
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
    let selection = json!({"flow":parent.key,"entry":"main","bridges":["integration"],"flowHashes":{parent.key.clone():parent.hash,child.key.clone():child.hash}});
    let (status, prepared) = request(
        &app,
        "POST",
        "/api/runtime-graphs/prepare",
        Some(json!({"workspaceId":workspace.id,"selection":selection})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{prepared}");
    assert_eq!(
        prepared["runtime"]["graph"]["types"]["SharedText"],
        json!({"kind":"text"})
    );
    let (status, repeated) = request(
        &app,
        "POST",
        "/api/runtime-graphs/prepare",
        Some(json!({"workspaceId":workspace.id,"selection":selection})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{repeated}");
    assert_eq!(repeated, prepared);
    assert_eq!(
        prepared["runtime"]["graph"]["routes"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    let (status,started)=request(&app,"POST","/api/runs",Some(json!({"workspaceId":workspace.id,"runtimeSelection":selection,"input":{"input":"question"}}))).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let id = started["id"].as_str().unwrap();
    let mut completed = Value::Null;
    for _ in 0..200 {
        let (status, run) = request(
            &app,
            "GET",
            &format!("/api/runs/{id}?workspaceId={}", workspace.id),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{run}");
        if run["status"] != "running" {
            completed = run;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(completed["status"], "completed", "{completed}");
    assert_eq!(
        completed["state"]["response"], "child result",
        "{completed}"
    );
    assert_eq!(
        completed["runtimeGraph"]["flows"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert!(
        completed["activities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["path"] == "integration/worker/action")
    );
    let (status, source) = request(
        &app,
        "GET",
        &format!("/api/runs/{id}/flow-source?workspaceId={}", workspace.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(source["hash"], completed["executedSourceHash"]);
    let mut pinned = selection;
    pinned["bridgeHashes"] = prepared["runtime"]["definitions"]["bridgeHashes"].clone();
    let bridges = BridgeStore::new(workspace.path.clone()).unwrap();
    let prior = bridges.read("integration").await.unwrap();
    let mut changed = bridge;
    let connection = changed.connections.remove("work").unwrap();
    changed
        .connections
        .insert("another-route".into(), connection);
    bridges
        .save("integration", &changed, Some(&prior.hash))
        .await
        .unwrap();
    let (status,conflict)=request(&app,"POST","/api/runs",Some(json!({"workspaceId":workspace.id,"runtimeSelection":pinned,"input":{"input":"question"}}))).await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
}

#[tokio::test]
async fn routed_child_wait_resumes_at_its_checkpoint_without_repeating_a_shell_effect() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let home = root.path().join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir(&home).unwrap();
    let app =
        zf_serve::server::router_with_home(root.path().join("data"), workspace.clone(), vec![], {
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
    parent.nodes[0].data.config["exports"]["interactive"] = json!(true);
    let mut child = flow("child", false);
    child.nodes[0].data.config["exports"]["interactive"] = json!(true);
    child.nodes[1].data.kind = "tool".into();
    child.nodes[1].data.config =
        json!({"tool":"exec","arguments":{"command":"printf x >> visits"}});
    child.nodes.push(serde_json::from_value(json!({"id":"question","position":{"x":24,"y":0},"data":{"kind":"input","label":"Question","config":{"field":"output","prompt":"Réponse ?","responseType":"text"}}})).unwrap());
    child
        .edges
        .iter_mut()
        .find(|e| e.source == "action")
        .unwrap()
        .target = "question".into();
    child.edges.push(
        serde_json::from_value(json!({"id":"d","source":"question","target":"publish"})).unwrap(),
    );
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
    let (status,started)=request(&app,"POST","/api/runs",Some(json!({"workspaceId":workspace.id,"runtimeSelection":selection,"input":{"input":"question"}}))).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let id = started["id"].as_str().unwrap();
    let waiting = wait_for(&app, id, &workspace.id).await;
    assert_eq!(waiting["status"], "waiting", "{waiting}");
    assert_eq!(waiting["wait"]["kind"], "input", "{}", waiting["wait"]);
    assert_eq!(waiting["wait"]["nodePath"], "integration/worker/question");
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("visits")).unwrap(),
        "x"
    );
    // The frozen runtime, nested checkpoints and route records are portable. Import
    // leaves execution paused and must not repeat the command in the new workspace.
    let (status, exported) = request(
        &app,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":workspace.id,"sessionIds":[id]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{exported}");
    let target_workspace = root.path().join("target");
    std::fs::create_dir(&target_workspace).unwrap();
    let imported_app = zf_serve::server::router_with_home(
        root.path().join("import-data"),
        target_workspace.clone(),
        vec![],
        {
            let home = root.path().join("import-home");
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await
    .unwrap();
    let (_, target_workspaces) = request(&imported_app, "GET", "/api/workspaces", None).await;
    let target_id = target_workspaces[0]["id"].as_str().unwrap();
    let (status, imported) = request(
        &imported_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":target_id,"path":exported["exports"][0]["path"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["runs"][0]["status"], "waiting");
    assert_eq!(
        imported["runs"][0]["import"]["resumeBlocked"],
        json!([]),
        "{imported}"
    );
    assert!(!target_workspace.join("visits").exists());
    let (status, answer) = request(
        &imported_app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={target_id}"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"imported continuation"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let imported_complete = wait_for(&imported_app, id, target_id).await;
    assert_eq!(
        imported_complete["status"], "completed",
        "{imported_complete}"
    );
    assert_eq!(
        imported_complete["state"]["response"],
        "imported continuation"
    );
    assert!(
        !target_workspace.join("visits").exists(),
        "Recorded effect must not run in the imported workspace"
    );
    let (status, answer) = request(
        &app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={}", workspace.id),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"continued"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let completed = wait_for(&app, id, &workspace.id).await;
    assert_eq!(completed["status"], "completed", "{completed}");
    assert_eq!(completed["state"]["response"], "continued");
    assert_eq!(
        std::fs::read_to_string(workspace.path.join("visits")).unwrap(),
        "x"
    );
    let (status, _) = request(
        &app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={}", workspace.id),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"stale"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
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
