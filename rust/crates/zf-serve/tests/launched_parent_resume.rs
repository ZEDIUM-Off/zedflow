use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;
use zf_core::identity::Scope;
use zf_flows::composition::BridgeDefinition;
use zf_flows::composition::Connection;
use zf_flows::composition::Endpoint;
use zf_flows::composition::InvocationKind;
use zf_flows::composition::RouteMode;
use zf_flows::schema::Composition;
use zf_storage::bridge_store::BridgeStore;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataRegistry;
use zf_storage::flow_store::FlowStore;

async fn request(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
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
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn chain(id: &str, exports: Value, mut nodes: Vec<Value>) -> Composition {
    nodes.insert(0, node("start", "start", json!({"exports":exports})));
    nodes.push(node("end", "end", json!({})));
    let edges: Vec<_> = nodes
        .windows(2)
        .enumerate()
        .map(|(i, pair)| json!({"id":i.to_string(),"source":pair[0]["id"],"target":pair[1]["id"]}))
        .collect();
    serde_json::from_value(json!({"formatVersion":3,"id":id,"name":id,"channels":[{"name":"visit","reducer":"overwrite"}],"nodes":nodes,"edges":edges})).unwrap()
}
fn exports(root: bool, interactive: bool) -> Value {
    let contract = json!({"input":{"kind":"text"},"output":{"kind":"text"}});
    let mut value = json!({"contract":{"entries":{"main":contract}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":interactive});
    if root {
        value["contract"]["branches"] =
            json!({"work":{"contract":contract,"invocations":["node"]}});
        value["branches"] = json!({"work":"launch"});
    } else {
        value["contract"]["data"] =
            json!({"answer":{"dataType":{"kind":"text"},"permissions":{"read":true,"write":true}}});
        value["data"] = json!({"answer":"response"});
    }
    value
}
async fn boundary(app: &Router, id: &str, workspace: &str) -> Value {
    let mut latest = Value::Null;
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let run = request(app, "GET", &format!("/api/runs/{id}?workspaceId={workspace}"), None).await;
            if run["status"] != "running" { return run; }
            latest = json!({"status":run["status"],"wait":run["wait"],"error":run["error"],"activities":run["activities"].as_array().into_iter().flatten().map(|a|json!({"path":a["path"],"status":a["status"]})).collect::<Vec<_>>()});
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await;
    result.unwrap_or_else(|error| panic!("Parent boundary must be visible without waiting for its launched child: {error}; {latest}"))
}
async fn file(path: &std::path::Path) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn answer(app: &Router, id: &str, workspace: &str, wait: &Value, text: &str) {
    request(
        app,
        "POST",
        &format!("/api/runs/{id}/answer?workspaceId={workspace}"),
        Some(json!({"waitId":wait["wait"]["id"],"value":text})),
    )
    .await;
}

#[tokio::test]
async fn parent_answers_twice_while_launched_child_runs_with_one_owner_and_all_publications_observed()
 {
    let temp = tempfile::tempdir().unwrap();
    let workspace_path = temp.path().join("workspace");
    std::fs::create_dir(&workspace_path).unwrap();
    let home = temp.path().join("home");
    let data = temp.path().join("data");
    let app = zf_serve::server::router_with_home(data.clone(), workspace_path.clone(), vec![], {
        let home = home.clone();
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let workspaces = request(&app, "GET", "/api/workspaces", None).await;
    let workspace: zf_storage::workspaces::Workspace =
        serde_json::from_value(workspaces[0].clone()).unwrap();
    let parent = chain(
        "parent",
        exports(true, true),
        vec![
            node("launch", "route", json!({"branch":"work","field":"visit"})),
            node(
                "allow-child-output",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":200}}),
            ),
            node(
                "first",
                "input",
                json!({"field":"input","prompt":"First answer"}),
            ),
            node(
                "release-first",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf p >> parent-effects; touch release-first"}}),
            ),
            node(
                "second",
                "input",
                json!({"field":"input","prompt":"Second answer"}),
            ),
            node(
                "release-second",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf p >> parent-effects; touch release-second"}}),
            ),
            node(
                "join",
                "await_route",
                json!({"inputField":"visit","field":"output"}),
            ),
            node("publish", "output", json!({"inputField":"output"})),
        ],
    );
    let mut child_nodes = vec![
        node(
            "first-effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":"printf x >> child-effects; touch child-started; while [ ! -f release-first ]; do printf tick; sleep 0.02; done"}}),
        ),
        node(
            "second-effect",
            "tool",
            json!({"tool":"exec","arguments":{"command":"printf x >> child-effects; touch child-second; while [ ! -f release-second ]; do sleep 0.01; done"}}),
        ),
    ];
    // More than one observation channel capacity must survive both resumes.
    for index in 0..24 {
        child_nodes.push(node(
            &format!("result-{index}"),
            "set",
            json!({"field":"output","value":"child result"}),
        ));
    }
    child_nodes.push(node("publish", "output", json!({"inputField":"output"})));
    let child = chain("worker", exports(false, false), child_nodes);
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
                RouteMode::Launch,
                InvocationKind::Node,
            ),
        );
    BridgeStore::new(workspace.path.clone())
        .unwrap()
        .save("async", &bridge, None)
        .await
        .unwrap();
    let started = request(&app, "POST", "/api/runs", Some(json!({"workspaceId":workspace.id,"runtimeSelection":{"flow":parent.key,"entry":"main","bridges":["async"]},"input":{"input":"initial"}}))).await;
    let id = started["id"].as_str().unwrap();
    let first = boundary(&app, id, &workspace.id).await;
    assert_eq!(first["status"], "waiting", "{}", first["error"]);
    assert_eq!(first["wait"]["nodePath"], "root/first");
    file(&workspace_path.join("child-started")).await;
    assert!(!workspace_path.join("release-first").exists());
    answer(&app, id, &workspace.id, &first, "continue first").await;
    let second = boundary(&app, id, &workspace.id).await;
    assert_eq!(
        second["wait"]["nodePath"], "root/second",
        "status={} error={} wait={}",
        second["status"], second["error"], second["wait"]
    );
    file(&workspace_path.join("child-second")).await;
    assert!(!workspace_path.join("release-second").exists());
    answer(&app, id, &workspace.id, &second, "continue second").await;
    let completed = boundary(&app, id, &workspace.id).await;
    assert_eq!(completed["status"], "completed", "{}", completed["error"]);
    assert!(completed["wait"].is_null());
    assert_eq!(completed["state"]["response"], "child result");
    assert_eq!(
        std::fs::read_to_string(workspace_path.join("child-effects")).unwrap(),
        "xx"
    );
    assert_eq!(
        std::fs::read_to_string(workspace_path.join("parent-effects")).unwrap(),
        "pp"
    );
    let activities = completed["activities"].as_array().unwrap();
    for index in 0..24 {
        assert_eq!(
            activities
                .iter()
                .filter(|a| a["path"] == format!("async/worker/result-{index}")
                    && a["status"] == "completed")
                .count(),
            1
        );
    }
    assert!(
        !activities
            .iter()
            .any(|a| a["status"] == "running" || a["status"] == "waiting")
    );
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite://{}", data.join("zedflow.db").display()))
            .await
            .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let registry = DataRegistry::new(pool, content.clone(), id).await.unwrap();
    assert_eq!(
        *registry
            .snapshot(&Scope::Flow("async/worker".into()), "answer")
            .await
            .unwrap()
            .value,
        json!("child result")
    );
    let visits = content
        .records(id)
        .await
        .unwrap()
        .into_iter()
        .filter(|record| record.kind == "route-visits")
        .count();
    assert_eq!(
        visits, 1,
        "Answer must reuse the existing child owner and visit"
    );
}

#[tokio::test]
async fn parent_releases_its_owner_when_child_is_paused_then_resumes_child_without_repeating_effect()
 {
    let temp = tempfile::tempdir().unwrap();
    let workspace_path = temp.path().join("workspace");
    std::fs::create_dir(&workspace_path).unwrap();
    let home = temp.path().join("home");
    let app = zf_serve::server::router_with_home(
        temp.path().join("data"),
        workspace_path.clone(),
        vec![],
        {
            let home = home.clone();
            std::fs::create_dir_all(&home).unwrap();
            home
        },
    )
    .await
    .unwrap();
    let workspaces = request(&app, "GET", "/api/workspaces", None).await;
    let workspace: zf_storage::workspaces::Workspace =
        serde_json::from_value(workspaces[0].clone()).unwrap();
    let parent = chain(
        "parent",
        exports(true, true),
        vec![
            node("launch", "route", json!({"branch":"work","field":"visit"})),
            node(
                "parent-question",
                "input",
                json!({"field":"input","prompt":"Parent question"}),
            ),
            node(
                "join",
                "await_route",
                json!({"inputField":"visit","field":"output"}),
            ),
            node("publish", "output", json!({"inputField":"output"})),
        ],
    );
    let child = chain(
        "worker",
        exports(false, true),
        vec![
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> effects"}}),
            ),
            node(
                "child-question",
                "input",
                json!({"field":"output","prompt":"Child question"}),
            ),
            node("publish", "output", json!({"inputField":"output"})),
        ],
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
                RouteMode::Launch,
                InvocationKind::Node,
            ),
        );
    BridgeStore::new(workspace.path.clone())
        .unwrap()
        .save("async", &bridge, None)
        .await
        .unwrap();
    let started = request(&app, "POST", "/api/runs", Some(json!({"workspaceId":workspace.id,"runtimeSelection":{"flow":parent.key,"entry":"main","bridges":["async"]},"input":{"input":"initial"}}))).await;
    let id = started["id"].as_str().unwrap();
    let waiting = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let run = request(
                &app,
                "GET",
                &format!("/api/runs/{id}?workspaceId={}", workspace.id),
                None,
            )
            .await;
            if run["status"] == "waiting" && run["runtimeActive"] != true {
                break run;
            }
            assert_ne!(run["status"], "error", "{}", run["error"]);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(waiting["wait"]["nodePath"], "root/parent-question");
    assert_eq!(
        std::fs::read_to_string(workspace_path.join("effects")).unwrap(),
        "x"
    );
    answer(&app, id, &workspace.id, &waiting, "join child").await;
    let child_wait = boundary(&app, id, &workspace.id).await;
    assert_eq!(
        child_wait["wait"]["nodePath"], "async/worker/child-question",
        "{}",
        child_wait["wait"]
    );
    answer(&app, id, &workspace.id, &child_wait, "child answer").await;
    let completed = boundary(&app, id, &workspace.id).await;
    assert_eq!(completed["status"], "completed", "{}", completed["error"]);
    assert_eq!(completed["state"]["response"], "child answer");
    assert_eq!(
        std::fs::read_to_string(workspace_path.join("effects")).unwrap(),
        "x"
    );
}
