mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use tower::ServiceExt;

async fn app(root: &Path) -> Router {
    app_with_service(root).await.0
}
async fn app_with_service(root: &Path) -> (Router, zf_execution::service::ExecutionService) {
    std::fs::create_dir_all(root.join("workspace")).unwrap();
    std::fs::write(
        root.join("workspace/AGENTS.md"),
        "Instructions figées : utiliser les fixtures.",
    )
    .unwrap();
    support::open_router(root.join("data"), root.join("workspace"), vec![], {
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap()
}
async fn raw(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(
                    body.map(|body| body.to_string()).unwrap_or_default(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        to_bytes(response.into_body(), 300_000_000)
            .await
            .unwrap()
            .to_vec(),
    )
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, bytes) = raw(app, method, path, body).await;
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&bytes))),
    )
}
async fn ok(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let (status, value) = request(app, method, path, body).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}
async fn wait(app: &Router, id: &str, status: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let run = ok(app, "GET", &format!("/api/runs/{id}"), None).await;
            assert_ne!(run["status"], "error", "{}", run["error"]);
            if run["status"] == status {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}})
}
fn flow(id: &str, nodes: Vec<Value>, edges: &[(&str, &str)]) -> Value {
    json!({"id":id,"name":id,"nodes":nodes,"edges":edges.iter().enumerate().map(|(index,(source,target))|json!({"id":index.to_string(),"source":source,"target":target})).collect::<Vec<_>>()})
}
fn nested() -> Value {
    let child = flow(
        "child",
        vec![
            node("s", "start", json!({})),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> effect.txt; printf 'sortie conservée'"}}),
            ),
            node("m", "agent", json!({"modelBinding":"runtime"})),
            node("out", "output", json!({"text":"{{output}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "effect"), ("effect", "m"), ("m", "out"), ("out", "e")],
    );
    flow(
        "parent",
        vec![
            node("s", "start", json!({})),
            node("child", "subgraph", json!({"composition":child})),
            node("out", "output", json!({"text":"{{output}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "child"), ("child", "out"), ("out", "e")],
    )
}
async fn start(app: &Router, composition: Value) -> Value {
    let run = ok(
        app,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition,"input":{"input":"Tâche à partager"}})),
    )
    .await;
    wait(app, run["id"].as_str().unwrap(), "waiting").await
}
async fn export(app: &Router, run: &Value) -> Value {
    ok(
        app,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":run["workspaceId"],"sessionIds":[run["id"]]})),
    )
    .await
}
fn rehash(directory: &Path) {
    let path = directory.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for file in manifest["files"].as_array_mut().unwrap() {
        let bytes = std::fs::read(directory.join(file["path"].as_str().unwrap())).unwrap();
        file["bytes"] = json!(bytes.len());
        file["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    }
    let inventory: Vec<zf_storage::session_archive::InventoryFile> =
        serde_json::from_value(manifest["files"].clone()).unwrap();
    manifest["archiveHash"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&inventory).unwrap())
    ));
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

async fn archive_record(directory: &Path, kind: &str, key: Option<&str>) -> Value {
    use zf_storage::content_store::ContentBlob;
    use zf_storage::content_store::ContentStore;
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(db).await.unwrap();
    let blobs: Vec<ContentBlob> = std::fs::read_to_string(directory.join("contents.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    store.import_blobs(&blobs).await.unwrap();
    let lines: Vec<Value> = std::fs::read_to_string(directory.join("session.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let record = lines
        .iter()
        .find(|entry| {
            entry["type"] == "record"
                && entry["record"]["kind"] == kind
                && key.is_none_or(|key| entry["record"]["key"] == key)
        })
        .unwrap();
    store
        .resolve(record["record"]["valueRef"].as_str().unwrap())
        .await
        .unwrap()
}
async fn live_store(root: &Path) -> zf_storage::content_store::ContentStore {
    let db = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}?mode=rw",
        root.join("data/zedflow.db").display()
    ))
    .await
    .unwrap();
    zf_storage::content_store::ContentStore::from_pool(db)
}

#[tokio::test]
async fn zip_roundtrip_restores_child_checkpoint_assets_and_context_without_repeating_effects() {
    let source = tempfile::tempdir().unwrap();
    let source_app = app(source.path()).await;
    let original = start(&source_app, nested()).await;
    let id = original["id"].as_str().unwrap();
    let exported = export(&source_app, &original).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    assert!(directory.starts_with(source.path().join("workspace/.zedflow/sessions")));
    let text = std::fs::read_to_string(directory.join("session.jsonl")).unwrap();
    assert!(text.contains("pendingNodes"));
    assert!(text.contains("checkpointRef"));
    assert!(!text.contains("\"state\":"));
    assert!(text.contains(&format!("{id}/child@")));
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["version"], 3);
    assert!(
        manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "contents.jsonl")
    );
    assert_eq!(
        archive_record(directory, "receipts", None).await["status"],
        "completed"
    );
    let output = archive_record(directory, "tool-output", None).await;
    let output_value = if let Some(key) = output["fragmentKey"].as_str() {
        archive_record(directory, "tool-output-fragments", Some(key)).await
    } else {
        output["content"].clone()
    };
    assert_eq!(decoded_asset(&output_value), "sortie conservée".as_bytes());
    assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
    let unchanged = ok(
        &source_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":original["workspaceId"],"path":directory})),
    )
    .await;
    assert_eq!(unchanged["unchanged"], 1);
    let (status, zip) = raw(
        &source_app,
        "GET",
        exported["downloadUrl"].as_str().unwrap(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(zip.starts_with(b"PK"));
    let target = tempfile::tempdir().unwrap();
    let (target_app, service) = app_with_service(target.path()).await;
    // Occupy event identities so the import must actually translate its anchors.
    let existing = start(
        &target_app,
        flow(
            "existing",
            vec![
                node("s", "start", json!({})),
                node("input", "input", json!({})),
                node("end", "end", json!({})),
            ],
            &[("s", "input"), ("input", "end")],
        ),
    )
    .await;
    // The persisted wait precedes the owner's final queue check and lease
    // release. Import requires that owner to finish, not only its visible wait.
    tokio::time::timeout(
        Duration::from_secs(15),
        service.wait_idle(existing["id"].as_str().unwrap()),
    )
    .await
    .expect("existing session owner did not finish after reaching wait")
    .unwrap();
    let health = ok(&target_app, "GET", "/api/health", None).await;
    let zipped = target.path().join("chosen-session.zip");
    std::fs::write(&zipped, zip).unwrap();
    let imported = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":zipped})),
    )
    .await;
    assert_eq!(imported["imported"], 1);
    let restored = &imported["runs"][0];
    assert_eq!(restored["id"], original["id"]);
    assert_eq!(restored["wait"], original["wait"]);
    assert_eq!(restored["flowSource"], original["flowSource"]);
    assert_eq!(restored["messages"], original["messages"]);
    let previous = original["timeline"].as_array().unwrap();
    let translated = restored["timeline"].as_array().unwrap();
    assert_eq!(previous.len(), translated.len());
    for (old, new) in previous.iter().zip(translated) {
        assert_eq!(old["id"], new["id"]);
        if old["seq"].as_i64().unwrap() > 0 {
            assert!(new["seq"].as_i64().unwrap() > old["seq"].as_i64().unwrap());
        }
    }
    for (old, new) in original["activities"]
        .as_array()
        .unwrap()
        .iter()
        .zip(restored["activities"].as_array().unwrap())
    {
        for field in ["startedSeq", "endedSeq"] {
            if let Some(seq) = old[field].as_i64() {
                assert!(new[field].as_i64().unwrap() > seq);
            }
        }
    }
    assert_eq!(restored["import"]["resumeBlocked"], json!([]));
    assert_eq!(
        restored["import"]["sourceWorkspace"]["path"],
        original["workspacePath"]
    );
    assert!(!target.path().join("workspace/effect.txt").exists());
    let output = restored["toolActivities"][0]["result"]["fullOutputPath"]
        .as_str()
        .unwrap();
    assert!(Path::new(output).starts_with(target.path().join("data/runs")));
    assert_eq!(std::fs::read_to_string(output).unwrap(), "sortie conservée");
    let local_instruction = restored["context"]["instructions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["content"] == "Instructions figées : utiliser les fixtures.")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(local_instruction["path"].as_str().unwrap()).unwrap(),
        local_instruction["content"].as_str().unwrap()
    );
    // A session shared a second time still exports paths local to its current
    // daemon and imports its complete output without consulting the first host.
    let forwarded = export(&target_app, restored).await;
    let forwarded_dir = Path::new(forwarded["exports"][0]["path"].as_str().unwrap());
    let forwarded_output = archive_record(forwarded_dir, "tool-output", None).await;
    assert!(
        Path::new(forwarded_output["path"].as_str().unwrap())
            .starts_with(target.path().join("data/runs"))
    );
    let third = tempfile::tempdir().unwrap();
    let third_app = app(third.path()).await;
    let third_health = ok(&third_app, "GET", "/api/health", None).await;
    let third_import = ok(
        &third_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":third_health["defaultWorkspaceId"],"path":forwarded_dir})),
    )
    .await;
    let third_output = third_import["runs"][0]["toolActivities"][0]["result"]["fullOutputPath"]
        .as_str()
        .unwrap();
    assert!(Path::new(third_output).starts_with(third.path().join("data/runs")));
    assert_eq!(
        std::fs::read_to_string(third_output).unwrap(),
        "sortie conservée"
    );
    let again = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":zipped})),
    )
    .await;
    assert_eq!(again["unchanged"], 1);
    assert_eq!(again["imported"], 0);
    service.shutdown().await.unwrap();
    drop(target_app);
    drop(service);
    let (restarted, service) = app_with_service(target.path()).await;
    let run = ok(&restarted, "GET", &format!("/api/runs/{id}"), None).await;
    assert_eq!(run["wait"], original["wait"]);
    ok(
        &restarted,
        "POST",
        &format!("/api/runs/{id}/answer"),
        Some(json!({"waitId":run["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}})),
    )
    .await;
    let completed = wait(&restarted, id, "completed").await;
    assert!(
        completed["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["role"] == "assistant")
    );
    assert!(
        !target.path().join("workspace/effect.txt").exists(),
        "the imported child must resume after the already completed effect"
    );
    assert_eq!(
        std::fs::read_to_string(source.path().join("workspace/effect.txt")).unwrap(),
        "x"
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn scoped_read_commands_streams_and_exports_cannot_cross_workspaces() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    let run = start(&router, nested()).await;
    let id = run["id"].as_str().unwrap();
    std::fs::create_dir(root.path().join("other")).unwrap();
    let other = ok(
        &router,
        "POST",
        "/api/workspaces",
        Some(json!({"path":root.path().join("other")})),
    )
    .await;
    let scope = other["id"].as_str().unwrap();
    for (method, suffix, body) in [
        ("GET", "", None),
        ("GET", "/snapshot", None),
        ("GET", "/events", None),
        ("POST", "/rtc", Some(json!({}))),
        (
            "POST",
            "/answer",
            Some(
                json!({"waitId":run["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}),
            ),
        ),
        (
            "PATCH",
            "/models",
            Some(
                json!({"nodePath":"child/m","selection":{"provider":"fixture","model":"fixture"},"revision":0}),
            ),
        ),
    ] {
        let (status, _) = request(
            &router,
            method,
            &format!("/api/runs/{id}{suffix}?workspaceId={scope}"),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{suffix}");
    }
    let other_run=ok(&router,"POST","/api/runs",Some(json!({"workspaceId":scope,"composition":flow("other-session",vec![node("s","start",json!({})),node("input","input",json!({})),node("end","end",json!({}))],&[("s","input"),("input","end")])}))).await;
    let other_id = other_run["id"].as_str().unwrap();
    for (method, suffix) in [
        ("GET", ""),
        ("GET", "/snapshot"),
        ("GET", "/events"),
        ("POST", "/rtc"),
        ("POST", "/answer"),
        ("PATCH", "/models"),
    ] {
        let (status, error) = request(
            &router,
            method,
            &format!("/api/runs/{other_id}{suffix}"),
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"], "Session absente de ce workspace");
    }
    let scoped = ok(
        &router,
        "GET",
        &format!("/api/runs/{other_id}?workspaceId={scope}"),
        None,
    )
    .await;
    assert_eq!(scoped["workspaceId"], scope);
    let default_list = ok(&router, "GET", "/api/runs", None).await;
    assert!(
        default_list
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["workspaceId"] == run["workspaceId"])
    );
    let other_list = ok(
        &router,
        "GET",
        &format!("/api/runs?workspaceId={scope}"),
        None,
    )
    .await;
    assert_eq!(other_list.as_array().unwrap().len(), 1);
    for endpoint in ["generate", "build"] {
        let (status, error) = request(
            &router,
            "POST",
            &format!("/api/{endpoint}"),
            Some(json!({"runId":other_id})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(error["error"], "Session absente de ce workspace");
        let absent = request(
            &router,
            "POST",
            &format!("/api/{endpoint}"),
            Some(json!({"runId":"nonexistent-fixture-run"})),
        )
        .await;
        assert_eq!(absent, (status, error));
    }
    ok(
        &router,
        "POST",
        "/api/generate",
        Some(json!({"runId":other_id,"workspaceId":scope})),
    )
    .await;
    let (status, _) = request(
        &router,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":scope,"sessionIds":[id]})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let foreign = request(
        &router,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":scope,"sessionIds":[other_id,id]})),
    )
    .await;
    let absent = request(
        &router,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":scope,"sessionIds":[other_id,"nonexistent-fixture-run"]})),
    )
    .await;
    assert_eq!(foreign.0, StatusCode::NOT_FOUND);
    assert_eq!(
        foreign.1,
        json!({"error":"Session absente de ce workspace"})
    );
    assert_eq!(foreign, absent);
    assert!(
        !root.path().join("data/session-downloads").exists(),
        "refused requests must not publish an archive"
    );
    assert!(!root.path().join("other/.zedflow/sessions").exists());
    assert!(!root.path().join("workspace/.zedflow/sessions").exists());
    assert!(!root.path().join("data/builds").exists());
    let exported = export(&router, &run).await;
    let wrong = exported["downloadUrl"]
        .as_str()
        .unwrap()
        .replace(run["workspaceId"].as_str().unwrap(), scope);
    let foreign_download = request(&router, "GET", &wrong, None).await;
    assert_eq!(
        foreign_download,
        (
            StatusCode::NOT_FOUND,
            json!({"error":"Export absent de ce workspace"})
        )
    );
    let absent_url = format!(
        "/api/sessions/exports/00000000-0000-0000-0000-000000000000.zip?workspaceId={scope}"
    );
    assert_eq!(
        request(&router, "GET", &absent_url, None).await,
        foreign_download
    );
    let authorized_url = exported["downloadUrl"].as_str().unwrap();
    let archive_id = authorized_url
        .split('/')
        .next_back()
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let archive_path = root
        .path()
        .join("data/session-downloads")
        .join(format!("{archive_id}.zip"));
    let (status, bytes) = raw(&router, "GET", authorized_url, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, std::fs::read(&archive_path).unwrap());
    std::fs::remove_file(archive_path).unwrap();
    assert_eq!(
        request(&router, "GET", authorized_url, None).await,
        foreign_download
    );
    wait_for_owners(&service).await;
    let (status, value) = request(
        &router,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":scope,"path":exported["exports"][0]["path"]})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{value}");
    assert_eq!(
        ok(&router, "GET", &format!("/api/runs/{id}"), None).await["wait"],
        run["wait"]
    );
    assert!(
        std::fs::read_to_string(root.path().join("other/.zedflow/.gitignore"))
            .unwrap()
            .contains("/*.db")
    );
}

#[tokio::test]
async fn altered_inventory_is_rejected_before_writing_any_imported_state() {
    let source = tempfile::tempdir().unwrap();
    let router = app(source.path()).await;
    let run = start(&router, nested()).await;
    let exported = export(&router, &run).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    let mut text = std::fs::read_to_string(directory.join("session.jsonl")).unwrap();
    text.push_str("{}\n");
    std::fs::write(directory.join("session.jsonl"), text).unwrap();
    let target = tempfile::tempdir().unwrap();
    let target_app = app(target.path()).await;
    let health = ok(&target_app, "GET", "/api/health", None).await;
    let (status, error) = request(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":directory})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(error["error"].as_str().unwrap().contains("altéré"));
    assert_eq!(ok(&target_app, "GET", "/api/runs", None).await, json!([]));
    assert!(!target.path().join("data/runs").exists());
}

#[tokio::test]
async fn uncertain_receipts_are_preserved_and_imported_sessions_cannot_resume_them() {
    let source = tempfile::tempdir().unwrap();
    let router = app(source.path()).await;
    let run = start(&router, nested()).await;
    let store = live_store(source.path()).await;
    let record = store
        .records(run["id"].as_str().unwrap())
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.kind == "receipts")
        .unwrap();
    let mut content = store.resolve(&record.value_ref).await.unwrap();
    content["status"] = json!("started");
    store
        .put_record(&record.scope, &record.kind, &record.key, &content)
        .await
        .unwrap();
    let exported = export(&router, &run).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    let target = tempfile::tempdir().unwrap();
    let target_app = app(target.path()).await;
    let health = ok(&target_app, "GET", "/api/health", None).await;
    let imported = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":directory})),
    )
    .await;
    assert!(
        imported["runs"][0]["import"]["resumeBlocked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap().contains("incertain"))
    );
    let id = run["id"].as_str().unwrap();
    let (status, error) = request(
        &target_app,
        "POST",
        &format!("/api/runs/{id}/answer"),
        Some(json!({"waitId":run["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(
        ok(&target_app, "GET", &format!("/api/runs/{id}"), None).await["status"],
        "waiting"
    );
    assert!(!target.path().join("workspace/effect.txt").exists());
    let copied = live_store(target.path())
        .await
        .record(id, "receipts", &record.key)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(copied["status"], "started");
}

#[tokio::test]
async fn exporting_a_running_graph_waits_for_an_explicit_stable_boundary() {
    let root = tempfile::tempdir().unwrap();
    let router = app(root.path()).await;
    let composition = flow(
        "busy",
        vec![
            node("s", "start", json!({})),
            node(
                "delay",
                "tool",
                json!({"tool":"delay","arguments":{"milliseconds":500}}),
            ),
            node("input", "input", json!({})),
            node("end", "end", json!({})),
        ],
        &[("s", "delay"), ("delay", "input"), ("input", "end")],
    );
    let started = ok(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"composition":composition})),
    )
    .await;
    let (status, _) = request(
        &router,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":started["workspaceId"],"sessionIds":[started["id"]]})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let waiting = wait(&router, started["id"].as_str().unwrap(), "waiting").await;
    assert_eq!(
        export(&router, &waiting).await["exports"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn effective_agent_context_and_model_provenance_survive_export_after_source_edits() {
    let source = tempfile::tempdir().unwrap();
    let source_app = app(source.path()).await;
    std::fs::write(
        source.path().join("workspace/attached.txt"),
        "Version réellement transmise au modèle",
    )
    .unwrap();
    let mut composition = flow(
        "explicit-agent",
        vec![
            node("s", "start", json!({})),
            node(
                "m",
                "agent",
                json!({"modelBinding":"runtime","fixtureSteps":[{"text":"Réponse figée"}],"attachments":{"files":{"items":[{"id":"reference","path":"attached.txt"}]},"instructions":{"items":[{"id":"rules","source":{"kind":"text","text":"Consigne locale"}}]}}}),
            ),
            node("out", "output", json!({"text":"{{output}}"})),
            node("input", "input", json!({})),
            node("e", "end", json!({})),
        ],
        &[("s", "m"), ("m", "out"), ("out", "input"), ("input", "e")],
    );
    composition["formatVersion"] = json!(2);
    let run=ok(&source_app,"POST","/api/runs",Some(json!({"composition":composition,"modelBindings":{"m":{"provider":"fixture","model":"fixture"}},"input":{"input":"Analyser"}}))).await;
    let run = wait(&source_app, run["id"].as_str().unwrap(), "waiting").await;
    assert_eq!(run["contextSnapshots"].as_array().unwrap().len(), 1);
    let snapshot = &run["contextSnapshots"][0];
    assert!(
        snapshot["files"]
            .as_str()
            .unwrap()
            .contains("Version réellement transmise au modèle")
    );
    std::fs::write(
        source.path().join("workspace/attached.txt"),
        "Nouvelle version sur disque",
    )
    .unwrap();
    let exported = export(&source_app, &run).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    let invocation = snapshot["invocationId"].as_str().unwrap();
    let snapshot_content =
        archive_record(directory, "capability-snapshots", Some(invocation)).await;
    let model_content = archive_record(directory, "model-calls", Some(invocation)).await;
    // The read projection enriches the immutable capture with references and
    // dispatch status. Every original captured field must remain identical.
    for (field, value) in snapshot_content.as_object().unwrap() {
        assert_eq!(&snapshot[field], value, "captured field {field}");
    }
    assert!(snapshot["contentRef"].is_string());
    assert!(snapshot["rawRef"].is_string());
    assert_eq!(snapshot["requestBoundary"], "fixtureInput");
    assert_eq!(snapshot["requestStatus"], "sent");
    assert!(
        snapshot_content
            .to_string()
            .contains("Version réellement transmise au modèle")
    );
    let target = tempfile::tempdir().unwrap();
    let target_app = app(target.path()).await;
    let health = ok(&target_app, "GET", "/api/health", None).await;
    // Import still displays the session when a project dependency is missing.
    let missing = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":directory})),
    )
    .await;
    assert!(
        missing["runs"][0]["import"]["resumeBlocked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap().contains("attached.txt"))
    );
    assert_eq!(
        request(
            &target_app,
            "POST",
            &format!("/api/runs/{}/answer", run["id"].as_str().unwrap()),
            Some(json!({"waitId":run["wait"]["id"],"value":"Continuer"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    // Restoring project files and reimporting the identical archive only refreshes
    // compatibility metadata; it preserves the existing run and history.
    std::fs::write(
        target.path().join("workspace/attached.txt"),
        "Fichier du projet cible",
    )
    .unwrap();
    let imported = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":directory})),
    )
    .await;
    assert_eq!(imported["unchanged"], 1);
    let restored = &imported["runs"][0];
    assert_eq!(restored["contextSnapshots"], run["contextSnapshots"]);
    assert_eq!(restored["import"]["resumeBlocked"], json!([]));
    let store = live_store(target.path()).await;
    let id = run["id"].as_str().unwrap();
    assert_eq!(
        store
            .record(id, "capability-snapshots", invocation)
            .await
            .unwrap(),
        Some(snapshot_content)
    );
    assert_eq!(
        store.record(id, "model-calls", invocation).await.unwrap(),
        Some(model_content)
    );
}

#[tokio::test]
async fn content_hash_missing_closure_and_expansion_bomb_are_rejected_before_import() {
    let source = tempfile::tempdir().unwrap();
    let router = app(source.path()).await;
    let run = start(&router, nested()).await;
    let exported = export(&router, &run).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    let original = std::fs::read_to_string(directory.join("contents.jsonl")).unwrap();
    let session = std::fs::read_to_string(directory.join("session.jsonl")).unwrap();
    for fault in ["hash", "missing", "expansion"] {
        let mut blobs: Vec<Value> = original
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut lines: Vec<Value> = session
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        match fault {
            "hash" => blobs[0]["body"]["corrupted"] = json!(true),
            "missing" => {
                blobs.remove(0);
            }
            _ => {
                let mut body = json!({"kind":"scalar","value":"bomb"});
                let mut reference = String::new();
                for depth in 0..36 {
                    if depth > 0 {
                        body =
                            json!({"kind":"object","entries":{"left":reference,"right":reference}});
                    }
                    reference = format!(
                        "sha256:{:x}",
                        Sha256::digest(serde_json::to_vec(&body).unwrap())
                    );
                    blobs.push(json!({"reference":reference,"body":body}));
                }
                lines[0]["run"]["projectionRef"] = json!(reference);
            }
        }
        std::fs::write(
            directory.join("contents.jsonl"),
            blobs.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        std::fs::write(
            directory.join("session.jsonl"),
            lines.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
        rehash(directory);
        let target = tempfile::tempdir().unwrap();
        let target_app = app(target.path()).await;
        let health = ok(&target_app, "GET", "/api/health", None).await;
        let (status, error) = request(
            &target_app,
            "POST",
            "/api/sessions/import",
            Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":directory})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "fault {fault}: {error}");
        assert_eq!(ok(&target_app, "GET", "/api/runs", None).await, json!([]));
        assert!(!target.path().join("data/runs").exists());
    }
}

fn remove_new_refs(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for key in ["fullOutputRef", "requestRef", "recordRef", "storageVersion"] {
                fields.remove(key);
            }
            for child in fields.values_mut() {
                remove_new_refs(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                remove_new_refs(item);
            }
        }
        _ => {}
    }
}
fn decoded_asset(value: &Value) -> Vec<u8> {
    if let Some(text) = value.as_str() {
        text.as_bytes().to_vec()
    } else {
        zf_storage::content_store::decode_full_output(value).unwrap()
    }
}

async fn legacy_copy(source: &Path, target: &Path) {
    use zf_storage::content_store::ContentBlob;
    use zf_storage::content_store::ContentStore;
    use zf_storage::session_store;
    let store = ContentStore::new(sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap())
        .await
        .unwrap();
    let blobs: Vec<ContentBlob> = std::fs::read_to_string(source.join("contents.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    store.import_blobs(&blobs).await.unwrap();
    let mut lines: Vec<Value> = std::fs::read_to_string(source.join("session.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let projection = store
        .resolve(lines[0]["run"]["projectionRef"].as_str().unwrap())
        .await
        .unwrap();
    lines[0]["run"] = session_store::hydrate_run(&store, &projection)
        .await
        .unwrap();
    lines[0]["version"] = json!(1);
    let runtime = std::path::PathBuf::from(lines[0]["runtimeRoot"].as_str().unwrap());
    let mut files = std::collections::BTreeMap::<String, Vec<u8>>::new();
    for file in lines[0]["contextFiles"].as_array_mut().unwrap() {
        let value = store
            .resolve(file["contentRef"].as_str().unwrap())
            .await
            .unwrap();
        files.insert(
            file["archivePath"].as_str().unwrap().into(),
            decoded_asset(&value),
        );
        file.as_object_mut().unwrap().remove("contentRef");
    }
    let record_lines = lines
        .iter()
        .filter(|entry| entry["type"] == "record")
        .cloned()
        .collect::<Vec<_>>();
    let mut journal = vec![lines.remove(0)];
    for mut line in lines {
        match line["type"].as_str().unwrap() {
            "registry" => {
                assert!(line["registry"]["entities"].as_array().unwrap().is_empty());
            }
            "event" => {
                line["event"] = session_store::hydrate_event(&store, &line["event"])
                    .await
                    .unwrap();
                journal.push(line);
            }
            "checkpointRef" => {
                let checkpoint = store
                    .resolve(line["checkpoint"]["checkpointRef"].as_str().unwrap())
                    .await
                    .unwrap();
                journal.push(json!({"type":"checkpoint","checkpoint":checkpoint}));
            }
            "record" => {
                let record = &line["record"];
                let mut value = store
                    .resolve(record["valueRef"].as_str().unwrap())
                    .await
                    .unwrap();
                if record["kind"] == "tool-output" {
                    let relative = Path::new(value["path"].as_str().unwrap())
                        .strip_prefix(&runtime)
                        .unwrap();
                    let output = if let Some(key) = value["fragmentKey"].as_str() {
                        let entry = record_lines
                            .iter()
                            .find(|entry| {
                                entry["record"]["kind"] == "tool-output-fragments"
                                    && entry["record"]["key"] == key
                            })
                            .unwrap();
                        store
                            .resolve(entry["record"]["valueRef"].as_str().unwrap())
                            .await
                            .unwrap()
                    } else {
                        value["content"].clone()
                    };
                    files.insert(
                        format!("runtime/{}", relative.display()),
                        decoded_asset(&output),
                    );
                } else if record["kind"] != "tool-output-fragments" {
                    remove_new_refs(&mut value);
                    files.insert(
                        format!(
                            "runtime/{}/{}.json",
                            record["kind"].as_str().unwrap(),
                            record["key"].as_str().unwrap()
                        ),
                        serde_json::to_vec(&value).unwrap(),
                    );
                }
            }
            _ => panic!("unexpected archive entry"),
        }
    }
    for line in &mut journal {
        remove_new_refs(line);
    }
    files.insert(
        "session.jsonl".into(),
        journal
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>()
            .into_bytes(),
    );
    let inventory: Vec<zf_storage::session_archive::InventoryFile> = files
        .iter()
        .map(|(path, bytes)| zf_storage::session_archive::InventoryFile {
            path: path.clone(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        })
        .collect();
    let manifest = json!({"format":"zedflow-session","version":1,"sessionId":journal[0]["run"]["id"],"archiveHash":format!("{:x}",Sha256::digest(serde_json::to_vec(&inventory).unwrap())),"files":inventory});
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    for (name, bytes) in files {
        let path = target.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

#[tokio::test]
async fn legacy_v1_import_remains_resumable_without_repeating_completed_effects() {
    let source = tempfile::tempdir().unwrap();
    let router = app(source.path()).await;
    let run = start(&router, nested()).await;
    let exported = export(&router, &run).await;
    let directory = Path::new(exported["exports"][0]["path"].as_str().unwrap());
    let legacy = source.path().join("legacy-export");
    legacy_copy(directory, &legacy).await;
    let target = tempfile::tempdir().unwrap();
    let target_app = app(target.path()).await;
    let health = ok(&target_app, "GET", "/api/health", None).await;
    let imported = ok(
        &target_app,
        "POST",
        "/api/sessions/import",
        Some(json!({"workspaceId":health["defaultWorkspaceId"],"path":legacy})),
    )
    .await;
    assert_eq!(imported["imported"], 1);
    assert_eq!(imported["runs"][0]["import"]["resumeBlocked"], json!([]));
    let id = run["id"].as_str().unwrap();
    ok(&target_app,"POST",&format!("/api/runs/{id}/answer"),Some(json!({"waitId":imported["runs"][0]["wait"]["id"],"value":{"provider":"fixture","model":"fixture"}}))).await;
    wait(&target_app, id, "completed").await;
    assert!(!target.path().join("workspace/effect.txt").exists());
}

#[tokio::test]
async fn held_maintenance_still_returns_service_unavailable_for_archive_requests() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    let guard = service.try_begin_maintenance().unwrap();
    let (status, body) = request(
        &router,
        "POST",
        "/api/sessions/export",
        Some(json!({"workspaceId":service.default_workspace_id(),"sessionIds":["absent"]})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("Maintenance"),
        "{body}"
    );
    assert!(!root.path().join("data/session-downloads").exists());
    drop(guard);
    assert_eq!(
        request(
            &router,
            "POST",
            "/api/sessions/export",
            Some(json!({"workspaceId":service.default_workspace_id(),"sessionIds":["absent"]}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    service.shutdown().await.unwrap();
}

async fn wait_for_owners(service: &zf_execution::service::ExecutionService) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match service.try_begin_maintenance() {
                Ok(guard) => {
                    drop(guard);
                    return;
                }
                Err(error) => {
                    assert!(
                        matches!(
                            error.downcast_ref::<zf_execution::commands::ExecutionError>(),
                            Some(zf_execution::commands::ExecutionError::Busy(_))
                        ),
                        "{error:#}"
                    );
                    tokio::task::yield_now().await;
                }
            }
        }
    })
    .await
    .expect("run owners did not release their maintenance leases after reaching wait");
}

#[tokio::test]
async fn invalid_archive_download_metadata_does_not_disclose_paths_or_contents() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    let directory = root.path().join("data/session-downloads");
    std::fs::create_dir_all(&directory).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let metadata = directory.join(format!("{id}.json"));
    let url = format!(
        "/api/sessions/exports/{id}.zip?workspaceId={}",
        service.default_workspace_id()
    );
    let refused = (
        StatusCode::BAD_REQUEST,
        json!({"error":"Téléchargement de session impossible"}),
    );
    for invalid in [
        "private-invalid-metadata-body",
        "null",
        "{}",
        r#"{"workspaceId":0}"#,
        r#"{"workspaceId":""}"#,
    ] {
        std::fs::write(&metadata, invalid).unwrap();
        assert_eq!(request(&router, "GET", &url, None).await, refused);
    }
    #[cfg(unix)]
    {
        std::fs::remove_file(&metadata).unwrap();
        let external = root.path().join("outside-download-directory");
        let authorized_metadata = json!({"workspaceId":service.default_workspace_id()}).to_string();
        std::fs::write(&external, &authorized_metadata).unwrap();
        let zip = directory.join(format!("{id}.zip"));
        std::fs::write(&zip, "private-archive-body").unwrap();
        std::os::unix::fs::symlink(&external, &metadata).unwrap();
        assert_eq!(request(&router, "GET", &url, None).await, refused);
        assert_eq!(
            std::fs::read_to_string(&external).unwrap(),
            authorized_metadata
        );
        std::fs::remove_file(&metadata).unwrap();
        std::fs::write(&metadata, &authorized_metadata).unwrap();
        std::fs::remove_file(&zip).unwrap();
        std::os::unix::fs::symlink(&external, &zip).unwrap();
        assert_eq!(request(&router, "GET", &url, None).await, refused);
        std::fs::write(
            &metadata,
            json!({"workspaceId":"foreign-workspace"}).to_string(),
        )
        .unwrap();
        assert_eq!(
            request(&router, "GET", &url, None).await,
            (
                StatusCode::NOT_FOUND,
                json!({"error":"Export absent de ce workspace"})
            ),
            "workspace must be checked before accessing even an invalid foreign ZIP"
        );
    }
    let invalid = format!(
        "/api/sessions/exports/%2e%2e%2fescape.zip?workspaceId={}",
        service.default_workspace_id()
    );
    assert_eq!(request(&router, "GET", &invalid, None).await, refused);
    service.shutdown().await.unwrap();
}
