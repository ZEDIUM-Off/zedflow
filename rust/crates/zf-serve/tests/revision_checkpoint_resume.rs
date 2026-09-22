mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

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
    let value: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
async fn wait(app: &Router, id: &str, predicate: impl Fn(&Value) -> bool) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let run = request(app, "GET", &format!("/api/runs/{id}"), None).await;
            assert_ne!(run["status"], "error", "{}", run["error"]);
            if predicate(&run) {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn restart_resumes_the_adopted_structure_at_a_user_checkpoint_without_repeating_effects() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let data = temp.path().join("data");
    let home = temp.path().join("home");
    let (app, service) = support::open_router(data.clone(), workspace.clone(), vec![], {
        let home = home.clone();
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let doc = json!({"formatVersion":3,"id":"adopted","name":"Adopted","nodes":[
        node("start","start",json!({})),
        node("effect","tool",json!({"tool":"exec","arguments":{"command":"printf x >> effects; touch started; while [ ! -f release ]; do sleep 0.01; done"}})),
        node("gate","input",json!({"field":"input","prompt":"Continue?"})),
        node("result","set",json!({"field":"output","value":"{{input}}"})),
        node("end","end",json!({}))
    ],"edges":[{"id":"a","source":"start","target":"effect"},{"id":"b","source":"effect","target":"gate"},{"id":"c","source":"gate","target":"result"},{"id":"d","source":"result","target":"end"}]});
    let saved = request(&app, "POST", "/api/flows", Some(json!({"composition":doc}))).await;
    let started = request(
        &app,
        "POST",
        "/api/runs",
        Some(json!({"flowKey":saved["key"],"flowHash":saved["hash"],"input":{"input":"initial"}})),
    )
    .await;
    let id = started["id"].as_str().unwrap();
    wait(&app, id, |run| {
        run["activities"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["path"] == "effect" && a["status"] == "running")
    })
    .await;
    let mut next = saved["composition"].clone();
    next["nodes"].as_array_mut().unwrap().push(node(
        "inserted",
        "set",
        json!({"field":"input","value":"adopted structure"}),
    ));
    next["edges"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|edge| edge["id"] == "c")
        .unwrap()["target"] = json!("inserted");
    next["edges"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"inserted-result","source":"inserted","target":"result"}));
    let published = request(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"key":saved["key"],"expectedHash":saved["hash"],"composition":next})),
    )
    .await;
    std::fs::write(workspace.join("release"), "release").unwrap();
    let waiting = wait(&app, id, |run| {
        run["status"] == "waiting" && run["runtimeActive"] != true
    })
    .await;
    assert_eq!(waiting["wait"]["nodePath"], "gate");
    let gate = waiting["activities"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|a| a["path"] == "gate")
        .unwrap();
    assert_eq!(gate["flowRevision"]["hash"], published["sourceHash"]);
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (reopened, service) = support::open_router(data, workspace.clone(), vec![], {
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    request(
        &reopened,
        "POST",
        &format!("/api/runs/{id}/answer"),
        Some(json!({"waitId":waiting["wait"]["id"],"value":"user answer"})),
    )
    .await;
    let completed = wait(&reopened, id, |run| run["status"] == "completed").await;
    assert_eq!(completed["state"]["output"], "adopted structure");
    assert_eq!(
        std::fs::read_to_string(workspace.join("effects")).unwrap(),
        "x"
    );
    let inserted = completed["activities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["path"] == "inserted")
        .collect::<Vec<_>>();
    assert_eq!(inserted.len(), 1);
    assert_eq!(inserted[0]["flowRevision"]["hash"], published["sourceHash"]);
    service.shutdown().await.unwrap();
}
