mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
async fn call(app: &Router, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().uri(path);
    if body.is_some() {
        req = req
            .method("POST")
            .header("content-type", "application/json");
    }
    let res = app
        .clone()
        .oneshot(
            req.body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 20_000_000).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
fn doc() -> Value {
    json!({"id":"test","name":"Interaction","revision":0,"nodes":[{"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Début"}},{"id":"ask","position":{"x":0,"y":0},"data":{"kind":"input","label":"Question","config":{"field":"input","prompt":"Votre nom ?","responseType":"text"}}},{"id":"reply","position":{"x":0,"y":0},"data":{"kind":"output","label":"Réponse","config":{"text":"Bonjour {{input}}"}}},{"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"Fin"}}],"edges":[{"id":"1","source":"s","target":"ask"},{"id":"2","source":"ask","target":"reply"},{"id":"3","source":"reply","target":"e"}]})
}
async fn wait(app: &Router, id: &str, status: &str) -> Value {
    for _ in 0..100 {
        let (_, run) = call(app, &format!("/api/runs/{id}"), None).await;
        if run["status"] == status {
            return run;
        }
        assert_ne!(run["status"], "error", "{run}");
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("Run did not reach {status}")
}
#[tokio::test]
async fn save_conflict_and_restart_then_answer_once() {
    let dir = tempfile::tempdir().unwrap();
    let (app, service) = support::open_router(dir.path().into(), dir.path().into(), vec![], {
        let home = dir.path().join("fixture-home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let (status, saved) = call(&app, "/api/compositions", Some(doc())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["revision"], 1);
    assert_eq!(
        call(&app, "/api/compositions", Some(doc())).await.0,
        StatusCode::CONFLICT
    );
    let (status, run) = call(
        &app,
        "/api/runs",
        Some(json!({"composition":saved,"input":{}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run}");
    let id = run["id"].as_str().unwrap();
    let waiting = wait(&app, id, "waiting").await;
    service.shutdown().await.unwrap();
    drop(app);
    drop(service);
    let (app, service) = support::open_router(dir.path().into(), dir.path().into(), vec![], {
        let home = dir.path().join("fixture-home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let answer = json!({"waitId":waiting["wait"]["id"],"value":"Ada"});
    let path = format!("/api/runs/{id}/answer");
    let (a, b) = tokio::join!(
        call(&app, &path, Some(answer.clone())),
        call(&app, &path, Some(answer))
    );
    assert!([a.0, b.0].contains(&StatusCode::OK));
    assert!([a.0, b.0].contains(&StatusCode::CONFLICT));
    let done = wait(&app, id, "completed").await;
    assert_eq!(done["state"]["response"], "Bonjour Ada");
    service.shutdown().await.unwrap();
}
#[tokio::test]
async fn generation_uses_adk_and_rejects_dangling_edges() {
    let dir = tempfile::tempdir().unwrap();
    let app = zf_serve::server::router_with_home(dir.path().into(), dir.path().into(), vec![], {
        let home = dir.path().join("fixture-home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap();
    let (status, code) = call(&app, "/api/generate", Some(doc())).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        code["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == "flows/instance-0/flow.rs")
            .unwrap()["content"]
            .as_str()
            .unwrap()
            .contains("graph.add_node_fn")
    );
    let mut broken = doc();
    broken["edges"][0]["target"] = json!("missing");
    assert_eq!(
        call(&app, "/api/generate", Some(broken)).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn isolated_subgraph_executes_and_exports_rust() {
    let mut child = doc();
    child["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|n| n["id"] != "ask");
    child["edges"] =
        json!([{"id":"1","source":"s","target":"reply"},{"id":"3","source":"reply","target":"e"}]);
    let mut parent = doc();
    parent["nodes"][1]["data"] =
        json!({"kind":"subgraph","label":"Enfant","config":{"composition":child}});
    parent["nodes"][2]["data"]["config"]["text"] = json!("Enfant : {{output}}");
    let composition: zf_flows::schema::Composition = serde_json::from_value(parent).unwrap();
    let result = zf_runtime::materialize::build(&composition)
        .unwrap()
        .invoke(
            adk_graph::State::from([("input".into(), json!("Ada"))]),
            adk_graph::ExecutionConfig::new("child-test"),
        )
        .await
        .unwrap();
    assert_eq!(result["response"], "Enfant : Bonjour Ada");
    let source = zf_flows::flow_source::render(
        &composition,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let composition = zf_flows::flow_source::parse(
        &source,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let generated = zf_compiler::export::export_single(
        &composition,
        &zf_flows::flow_source::render(
            &composition,
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
    assert!(
        std::str::from_utf8(&generated.files["flows/instance-0/flow.rs"])
            .unwrap()
            .contains("subgraphs::ResumableSubgraph::new")
    );
    let dir = tempfile::tempdir().unwrap();
    for (relative, content) in &generated.files {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    // Opt-in expensive compilation exercises the complete generated artifact.
    if std::env::var_os("ZEDFLOW_TEST_CODEGEN").is_some() {
        let output = std::process::Command::new("cargo")
            .args(["run", "--quiet", "--manifest-path"])
            .arg(dir.path().join("Cargo.toml"))
            .args(["--", "--workspace"])
            .arg(dir.path())
            .arg("--home")
            .arg(dir.path().join(".fixture-home"))
            .arg("--data")
            .arg(dir.path().join("data"))
            .args(["--input", r#"{"input":"Ada"}"#])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["status"], "completed", "{result}");
        assert_eq!(result["state"]["response"], "Enfant : Bonjour Ada");
    }
}
