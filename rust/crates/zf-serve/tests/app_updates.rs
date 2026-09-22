use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use zf_serve::app_updates::Config;
use zf_serve::app_updates::Updates;
use zf_serve::app_updates::build;
use zf_serve::app_updates::now;
async fn request(
    app: &Router,
    path: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path);
    for (name, value) in headers {
        req = req.header(*name, *value)
    }
    let req = req
        .header("content-type", "application/json")
        .body(
            body.map(|v| Body::from(v.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let code = response.status();
    (
        code,
        serde_json::from_slice(&to_bytes(response.into_body(), 4_000_000).await.unwrap()).unwrap(),
    )
}
async fn fixture(root: &std::path::Path) -> (Router, Arc<Updates>) {
    for dir in ["workspace", "home", "releases", "web"] {
        std::fs::create_dir_all(root.join(dir)).unwrap()
    }
    let current = "a".repeat(64);
    let candidate = "b".repeat(64);
    std::fs::create_dir_all(root.join("releases").join(&candidate)).unwrap();
    let metadata = build();
    std::fs::write(root.join("web/client-version.json"), metadata.to_string()).unwrap();
    std::fs::write(
        root.join("releases").join(&candidate).join("manifest.json"),
        json!({"format":1,"releaseId":candidate,"daemon":metadata,"client":metadata,"files":{"private-inventory":"x".repeat(100_000)}}).to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("candidate.json"),
        json!({"releaseId":candidate}).to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("manager.json"),
        json!({"releaseId":current,"updatedAt":now(),"ready":true}).to_string(),
    )
    .unwrap();
    let updates = Arc::new(Updates::new(Config {
        web: Some(root.join("web")),
        root: Some(root.to_owned()),
        release: Some(current),
    }));
    let app = zf_serve::server::router_with_runtime_and_updates(
        root.join("data"),
        root.join("workspace"),
        vec![],
        Some(root.join("home")),
        Some(root.join("home")),
        None,
        tokio_util::sync::CancellationToken::new(),
        updates.clone(),
    )
    .await
    .unwrap();
    (app, updates)
}
#[tokio::test]
async fn versions_refuse_stale_cross_origin_and_active_updates_then_freeze_mutations() {
    let root = tempfile::tempdir().unwrap();
    let (app, updates) = fixture(root.path()).await;
    let (code, status) = request(&app, "/api/version", None, &[]).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(status["daemon"], build());
    assert_eq!(status["managed"], true);
    assert!(status.to_string().len() < 5_000);
    assert!(status["candidate"].get("files").is_none());
    let update = json!({"releaseId":"b".repeat(64),"expectedDaemonBuildId":build()["buildId"]});
    let (code, _) = request(
        &app,
        "/api/updates/apply",
        Some(update.clone()),
        &[
            ("origin", "https://elsewhere.example"),
            ("host", "localhost:1234"),
        ],
    )
    .await;
    assert_eq!(code, StatusCode::FORBIDDEN);
    let mut stale = update.clone();
    stale["expectedDaemonBuildId"] = json!("old");
    assert_eq!(
        request(&app, "/api/updates/apply", Some(stale), &[])
            .await
            .0,
        StatusCode::CONFLICT
    );
    // A persisted status alone is not an execution owner. Hold a real tool
    // passage at an observable filesystem barrier, then let it reach input.
    let composition = json!({"id":"active","name":"Active update guard","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start"}},
        {"id":"effect","position":{"x":0,"y":0},"data":{"kind":"tool","label":"Barrier","config":{"tool":"exec","arguments":{"command":"touch update-started; while [ ! -f update-release ]; do sleep 0.01; done; printf done"}}}},
        {"id":"wait","position":{"x":0,"y":0},"data":{"kind":"input","label":"Wait","config":{"field":"input","prompt":"Continue?","responseType":"text"}}},
        {"id":"end","position":{"x":0,"y":0},"data":{"kind":"end","label":"End"}}
    ],"edges":[{"id":"a","source":"start","target":"effect"},{"id":"b","source":"effect","target":"wait"},{"id":"c","source":"wait","target":"end"}]});
    let (code, run) = request(
        &app,
        "/api/runs",
        Some(json!({"composition":composition,"input":{}})),
        &[],
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{run}");
    let run_id = run["id"].as_str().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !root.path().join("workspace/update-started").exists() {
            let (_, run) = request(&app, &format!("/api/runs/{run_id}"), None, &[]).await;
            assert_ne!(run["status"], "error", "{run}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("tool did not reach the update barrier");
    let (code, error) = request(&app, "/api/updates/apply", Some(update.clone()), &[]).await;
    assert_eq!(code, StatusCode::CONFLICT);
    assert!(error["error"].as_str().unwrap().contains("active"));
    assert!(!root.path().join("request.json").exists());
    std::fs::write(root.path().join("workspace/update-release"), "release").unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let (_, run) = request(&app, &format!("/api/runs/{run_id}"), None, &[]).await;
            assert_ne!(run["status"], "error", "{run}");
            if run["status"] == "waiting" && run["runtimeActive"] != true {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("execution owner did not release at the human wait");
    let (code, ack) = request(&app, "/api/updates/apply", Some(update.clone()), &[]).await;
    assert_eq!(code, StatusCode::OK, "{ack}");
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("request.json")).unwrap()).unwrap();
    assert_eq!(ack, persisted);
    assert!(*updates.gate.read().await);
    assert_eq!(
        request(&app, "/api/updates/apply", Some(update), &[])
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(
            &app,
            "/api/workspaces",
            Some(json!({"path":root.path().join("other")})),
            &[]
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        request(&app, "/api/version", None, &[]).await.0,
        StatusCode::OK
    );
}
#[tokio::test]
async fn incompatible_protocol_and_dead_supervisor_never_schedule_an_update() {
    let root = tempfile::tempdir().unwrap();
    let (app, _) = fixture(root.path()).await;
    assert_eq!(
        request(
            &app,
            "/api/workspaces",
            Some(json!({})),
            &[("x-zedflow-protocol", "999")]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    std::fs::write(
        root.path().join("manager.json"),
        json!({"releaseId":"a".repeat(64),"updatedAt":0}).to_string(),
    )
    .unwrap();
    let status = request(&app, "/api/version", None, &[]).await.1;
    assert_eq!(status["managed"], false);
    assert_eq!(
        request(
            &app,
            "/api/updates/apply",
            Some(json!({"releaseId":"b".repeat(64),"expectedDaemonBuildId":build()["buildId"]})),
            &[]
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert!(!root.path().join("request.json").exists());
}

async fn assert_rejected_update_releases_locks(
    app: &Router,
    updates: &Updates,
    root: &std::path::Path,
    body: Value,
    expected: StatusCode,
) {
    let (status, response) = request(app, "/api/updates/apply", Some(body), &[]).await;
    assert_eq!(status, expected, "{response}");
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        assert_eq!(
            response,
            json!({"error":"Impossible de préparer la mise à jour"})
        );
    }
    assert!(
        !*updates
            .gate
            .try_write()
            .expect("update gate was left locked")
    );
    assert!(!updates.requested.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!root.join("request.json").is_file());
    assert!(std::fs::read_dir(root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
    // A real mutating command proves that the execution maintenance lease dropped too.
    assert_eq!(
        request(
            app,
            "/api/workspaces",
            Some(json!({"path":root.join("workspace")})),
            &[]
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn update_business_conflicts_are_distinct_from_invalid_absent_and_technical_failures() {
    let root = tempfile::tempdir().unwrap();
    let (app, updates) = fixture(root.path()).await;
    let target = "b".repeat(64);
    let body = json!({"releaseId":target,"expectedDaemonBuildId":build()["buildId"]});
    let manifest_path = root
        .path()
        .join("releases")
        .join(&target)
        .join("manifest.json");
    let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    let manager_path = root.path().join("manager.json");
    let manager = std::fs::read(&manager_path).unwrap();
    let mut invalid = body.clone();
    invalid["releaseId"] = json!("../not-a-release");
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        invalid,
        StatusCode::BAD_REQUEST,
    )
    .await;
    let mut absent = body.clone();
    absent["releaseId"] = json!("c".repeat(64));
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        absent,
        StatusCode::NOT_FOUND,
    )
    .await;
    let mut active = body.clone();
    active["releaseId"] = json!("a".repeat(64));
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        active,
        StatusCode::CONFLICT,
    )
    .await;
    let mut stale = body.clone();
    stale["expectedDaemonBuildId"] = json!("old");
    assert_rejected_update_releases_locks(&app, &updates, root.path(), stale, StatusCode::CONFLICT)
        .await;
    for (field, value) in [
        ("storageEpoch", json!(999)),
        ("protocol", json!(999)),
        ("target", json!("other-platform")),
    ] {
        let mut changed = manifest.clone();
        changed["daemon"][field] = value;
        std::fs::write(&manifest_path, changed.to_string()).unwrap();
        assert_rejected_update_releases_locks(
            &app,
            &updates,
            root.path(),
            body.clone(),
            StatusCode::CONFLICT,
        )
        .await;
    }
    for contents in ["not json", r#"{"format":999}"#] {
        std::fs::write(&manifest_path, contents).unwrap();
        assert_rejected_update_releases_locks(
            &app,
            &updates,
            root.path(),
            body.clone(),
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .await;
    }
    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::create_dir(&manifest_path).unwrap();
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        body.clone(),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    std::fs::remove_dir(&manifest_path).unwrap();
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    std::fs::write(&manager_path, "not json").unwrap();
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        body.clone(),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    std::fs::remove_file(&manager_path).unwrap();
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        body.clone(),
        StatusCode::CONFLICT,
    )
    .await;
    std::fs::write(&manager_path, manager).unwrap();
    // Deterministic I/O failure even when tests run as a privileged user.
    std::fs::create_dir(root.path().join("request.json")).unwrap();
    assert_rejected_update_releases_locks(
        &app,
        &updates,
        root.path(),
        body.clone(),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    std::fs::remove_dir(root.path().join("request.json")).unwrap();
    let (status, ack) = request(&app, "/api/updates/apply", Some(body), &[]).await;
    assert_eq!(status, StatusCode::OK, "{ack}");
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(root.path().join("request.json")).unwrap())
            .unwrap(),
        ack
    );
}
