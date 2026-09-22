mod support;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use std::path::Path;
use tower::ServiceExt;

async fn call(app: &Router, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 20_000_000).await.unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)}));
    (status, value)
}
async fn ok(app: &Router, method: &str, path: &str, body: Option<Value>) -> Value {
    let (status, value) = call(app, method, path, body).await;
    assert_eq!(status, 200, "{method} {path}: {value}");
    value
}
async fn app(root: &Path) -> Router {
    app_with_service(root).await.0
}
async fn app_with_service(root: &Path) -> (Router, zf_execution::service::ExecutionService) {
    for dir in ["a", "b", "home"] {
        tokio::fs::create_dir_all(root.join(dir)).await.unwrap();
    }
    support::open_router(root.join("data"), root.join("a"), vec![], {
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        home
    })
    .await
    .unwrap()
}
fn doc(id: &str) -> Value {
    json!({"id":id,"name":id,"revision":0,"nodes":[
        {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Début"}},
        {"id":"ask","position":{"x":120,"y":0},"data":{"kind":"input","label":"Question","config":{"field":"input","prompt":"Continue ?"}}},
        {"id":"read","position":{"x":240,"y":0},"data":{"kind":"tool","label":"Lire","config":{"tool":"read","arguments":{"path":"marker.txt"},"field":"result"}}},
        {"id":"out","position":{"x":360,"y":0},"data":{"kind":"output","label":"Réponse","config":{"text":"{{result}}"}}},
        {"id":"e","position":{"x":480,"y":0},"data":{"kind":"end","label":"Fin"}}],
        "edges":[{"id":"1","source":"s","target":"ask"},{"id":"2","source":"ask","target":"read"},{"id":"3","source":"read","target":"out"},{"id":"4","source":"out","target":"e"}]})
}
async fn wait(app: &Router, id: &str, status: &str, workspace: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let run = ok(
                app,
                "GET",
                &format!("/api/runs/{id}?workspaceId={workspace}"),
                None,
            )
            .await;
            assert_ne!(run["status"], "error", "{run}");
            if run["status"] == status {
                return run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("run status timeout")
}

#[tokio::test]
async fn flows_are_files_discovered_in_four_roots_with_conflicts_and_no_database_catalog() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path()).await;
    let workspaces = ok(&app, "GET", "/api/workspaces", None).await;
    let a = workspaces[0]["id"].as_str().unwrap();
    let b = ok(
        &app,
        "POST",
        "/api/workspaces",
        Some(json!({"path":root.path().join("b")})),
    )
    .await;
    let b = b["id"].as_str().unwrap();
    let local = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":a,"composition":doc("local")})),
    )
    .await;
    let global = ok(
        &app,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":a,"scope":"global","composition":doc("global")})),
    )
    .await;
    assert!(
        Path::new(local["path"].as_str().unwrap()).starts_with(root.path().join("a/.zedflow/flow"))
    );
    assert!(
        Path::new(global["path"].as_str().unwrap())
            .starts_with(root.path().join("home/.zedflow/flow"))
    );
    for (directory, id) in [
        ("a/.agents/flows", "other-local"),
        ("home/.agents/flows", "other-global"),
    ] {
        let directory = root.path().join(directory);
        tokio::fs::create_dir_all(&directory).await.unwrap();
        let composition = serde_json::from_value(doc(id)).unwrap();
        tokio::fs::write(
            directory.join("shared.rs"),
            zf_flows::flow_source::render(
                &composition,
                &zf_compiler::graph_compiler::GraphValidator::new(
                    &zf_runtime::materialize::RuntimePrimitives,
                ),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    }
    let list_a = ok(&app, "GET", &format!("/api/flows?workspaceId={a}"), None).await;
    assert_eq!(list_a.as_array().unwrap().len(), 4);
    let list_b = ok(&app, "GET", &format!("/api/flows?workspaceId={b}"), None).await;
    assert_eq!(list_b.as_array().unwrap().len(), 2);
    assert!(
        list_b
            .as_array()
            .unwrap()
            .iter()
            .all(|flow| flow["scope"] == "global")
    );
    let alternate = list_a
        .as_array()
        .unwrap()
        .iter()
        .find(|flow| flow["id"] == "other-local")
        .unwrap();
    let legacy_source = tokio::fs::read(alternate["path"].as_str().unwrap())
        .await
        .unwrap();
    let converted = ok(
        &app,
        "POST",
        "/api/flows/convert",
        Some(json!({"workspaceId":a,"key":alternate["key"],"expectedHash":alternate["hash"]})),
    )
    .await;
    assert_eq!(converted["oldKey"], alternate["key"]);
    assert_ne!(converted["newKey"], alternate["key"]);
    let converted = &converted["flow"];
    assert_eq!(
        tokio::fs::read(Path::new(converted["path"].as_str().unwrap()).join("flow.rs"))
            .await
            .unwrap(),
        legacy_source
    );
    let mut renamed = converted["composition"].clone();
    renamed["name"] = json!("Renommé");
    let saved = ok(&app, "POST", "/api/flows", Some(json!({"workspaceId":a,"key":converted["key"],"expectedHash":converted["hash"],"composition":renamed}))).await;
    assert_eq!(saved["path"], converted["path"]);
    let path = Path::new(local["path"].as_str().unwrap()).join("flow.rs");
    let original = tokio::fs::read_to_string(&path).await.unwrap();
    tokio::fs::write(&path, format!("{original}\n// external edit\n"))
        .await
        .unwrap();
    let (status, _) = call(&app, "POST", "/api/flows", Some(json!({"workspaceId":a,"key":local["key"],"expectedHash":local["hash"],"composition":local["composition"]}))).await;
    assert_eq!(status, 409);
    // The old revision-only API must not bypass file hash conflicts.
    let (status, _) = call(
        &app,
        "POST",
        "/api/compositions",
        Some(local["composition"].clone()),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(
        tokio::fs::read_to_string(&path).await.unwrap(),
        format!("{original}\n// external edit\n")
    );
    assert_eq!(local["fileVersion"], 1);
    let invalid = root.path().join("a/.zedflow/flows/invalid.rs");
    tokio::fs::create_dir_all(invalid.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(&invalid, "fn main() { panic!(\"must never execute\"); }")
        .await
        .unwrap();
    let catalog = ok(&app, "GET", &format!("/api/flows?workspaceId={a}"), None).await;
    assert!(
        catalog
            .as_array()
            .unwrap()
            .iter()
            .any(|flow| flow["path"] == invalid.to_str().unwrap()
                && !flow["diagnostics"].as_array().unwrap().is_empty())
    );
    let db = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}",
        root.path().join("data/zedflow.db").display()
    ))
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='compositions'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let key = saved["key"].as_str().unwrap();
    let hash = saved["hash"].as_str().unwrap();
    ok(
        &app,
        "DELETE",
        &format!("/api/flows/{key}?workspaceId={a}&expectedHash={hash}"),
        None,
    )
    .await;
    assert!(
        !tokio::fs::try_exists(saved["path"].as_str().unwrap())
            .await
            .unwrap()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn catalog_diagnoses_special_files_without_reading_them() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path()).await;
    let directory = root.path().join("a/.zedflow/flows");
    tokio::fs::create_dir_all(&directory).await.unwrap();
    let fifo = directory.join("pipe.rs");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let catalog = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        ok(&app, "GET", "/api/flows", None),
    )
    .await
    .expect("catalog must not read a FIFO");
    let entry = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == fifo.to_str().unwrap())
        .unwrap();
    assert!(entry["composition"].is_null());
    assert!(!entry["diagnostics"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn sessions_keep_workspace_and_source_when_navigating_editing_closing_and_restarting() {
    let root = tempfile::tempdir().unwrap();
    let (router, service) = app_with_service(root.path()).await;
    for (directory, value) in [("a", "WORKSPACE_A"), ("b", "WORKSPACE_B")] {
        tokio::fs::write(root.path().join(directory).join("marker.txt"), value)
            .await
            .unwrap();
        tokio::fs::write(
            root.path().join(directory).join("AGENTS.md"),
            format!("Instructions {value}"),
        )
        .await
        .unwrap();
    }
    let workspaces = ok(&router, "GET", "/api/workspaces", None).await;
    let a = workspaces[0]["id"].as_str().unwrap();
    let b = ok(
        &router,
        "POST",
        "/api/workspaces",
        Some(json!({"path":root.path().join("b")})),
    )
    .await;
    let b = b["id"].as_str().unwrap();
    let mut reader = doc("reader");
    reader["nodes"].as_array_mut().unwrap().push(json!({
        "id":"effect","position":{"x":60,"y":120},
        "data":{"kind":"tool","label":"Effet avant la pause","config":{
            "tool":"exec","arguments":{"command":"printf x >> effects.txt"},"field":"effectResult"
        }}
    }));
    reader["edges"][0]["target"] = json!("effect");
    reader["edges"].as_array_mut().unwrap().push(json!({
        "id":"effect-ask","source":"effect","target":"ask"
    }));
    let flow = ok(
        &router,
        "POST",
        "/api/flows",
        Some(json!({"workspaceId":a,"scope":"global","composition":reader})),
    )
    .await;
    let mut waiting = Vec::new();
    for id in [a, b] {
        let run = ok(&router, "POST", "/api/runs", Some(json!({"workspaceId":id,"flowKey":flow["key"],"flowHash":flow["hash"],"input":{"input":"Lis mon dossier"}}))).await;
        let run = ok(
            &router,
            "GET",
            &format!("/api/runs/{}?workspaceId={id}", run["id"].as_str().unwrap()),
            None,
        )
        .await;
        assert_eq!(run["name"], "Lis mon dossier");
        assert_eq!(run["workspaceId"], id);
        let paused = wait(&router, run["id"].as_str().unwrap(), "waiting", id).await;
        assert_eq!(
            tokio::fs::read_to_string(
                Path::new(paused["workspacePath"].as_str().unwrap()).join("effects.txt")
            )
            .await
            .unwrap(),
            "x"
        );
        waiting.push(paused);
    }
    let mut changed = flow["composition"].clone();
    changed["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["id"] == "out")
        .unwrap()["data"]["config"]["text"] = json!("changed");
    let published = ok(&router, "POST", "/api/flows", Some(json!({"workspaceId":a,"key":flow["key"],"expectedHash":flow["hash"],"composition":changed}))).await;
    let published_source =
        tokio::fs::read_to_string(Path::new(published["path"].as_str().unwrap()).join("flow.rs"))
            .await
            .unwrap();
    assert_ne!(published["hash"], flow["hash"]);
    assert_eq!(
        zf_storage::flow_store::hash(published_source.as_bytes()),
        published["sourceHash"]
    );
    let exported = ok(
        &router,
        "POST",
        "/api/generate",
        Some(json!({"runId":waiting[0]["id"]})),
    )
    .await;
    let exported_source = exported["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "flows/instance-0/flow.rs")
        .unwrap();
    assert_eq!(exported_source["content"], waiting[0]["flowSource"]);
    assert_ne!(
        tokio::fs::read_to_string(Path::new(flow["path"].as_str().unwrap()).join("flow.rs"))
            .await
            .unwrap(),
        exported_source["content"].as_str().unwrap()
    );
    ok(
        &router,
        "PATCH",
        &format!("/api/workspaces/{b}"),
        Some(json!({"open":false})),
    )
    .await;
    let (status, _) = call(
        &router,
        "POST",
        "/api/runs",
        Some(json!({"workspaceId":b,"flowKey":flow["key"],"flowHash":flow["hash"]})),
    )
    .await;
    assert_eq!(status, 409);
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    let (router, service) = app_with_service(root.path()).await;
    let workspaces = ok(&router, "GET", "/api/workspaces", None).await;
    assert_eq!(
        workspaces
            .as_array()
            .unwrap()
            .iter()
            .find(|workspace| workspace["id"] == b)
            .unwrap()["open"],
        false
    );
    for (waiting, marker) in waiting.iter().zip(["WORKSPACE_A", "WORKSPACE_B"]) {
        let id = waiting["id"].as_str().unwrap();
        let workspace = waiting["workspaceId"].as_str().unwrap();
        ok(
            &router,
            "POST",
            &format!("/api/runs/{id}/answer?workspaceId={workspace}"),
            Some(json!({"waitId":waiting["wait"]["id"],"value":"oui"})),
        )
        .await;
        let done = wait(&router, id, "completed", workspace).await;
        assert_eq!(done["workspaceId"], waiting["workspaceId"]);
        assert_eq!(done["workspacePath"], waiting["workspacePath"]);
        assert_eq!(done["flowRef"]["hash"], flow["hash"]);
        assert_eq!(done["flowSource"], waiting["flowSource"]);
        assert_eq!(done["state"]["response"], "changed");
        assert!(
            done["state"]["result"]["content"]
                .as_str()
                .unwrap()
                .contains(marker),
            "{}",
            done["state"]
        );
        let activities = done["activities"].as_array().unwrap();
        let effects: Vec<_> = activities
            .iter()
            .filter(|activity| activity["path"] == "effect")
            .collect();
        assert_eq!(
            effects.len(),
            1,
            "The completed pre-checkpoint effect is not replayed"
        );
        let previous_effect = waiting["activities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|activity| activity["path"] == "effect")
            .unwrap();
        assert_eq!(effects[0]["occurrenceId"], previous_effect["occurrenceId"]);
        assert_eq!(effects[0]["flowRevision"]["hash"], flow["sourceHash"]);
        assert_eq!(effects[0]["status"], "completed");
        assert_eq!(
            tokio::fs::read_to_string(
                Path::new(done["workspacePath"].as_str().unwrap()).join("effects.txt")
            )
            .await
            .unwrap(),
            "x"
        );
        let output = activities
            .iter()
            .find(|activity| activity["path"] == "out")
            .unwrap();
        assert_eq!(output["flowRevision"]["hash"], published["sourceHash"]);
        for (activity, source, hash) in [
            (
                effects[0],
                waiting["flowSource"].as_str().unwrap(),
                &flow["sourceHash"],
            ),
            (output, published_source.as_str(), &published["sourceHash"]),
        ] {
            let definition = ok(
                &router,
                "GET",
                &format!(
                    "/api/runs/{id}/definition?workspaceId={workspace}&nodePath={}&occurrenceId={}",
                    activity["path"].as_str().unwrap(),
                    activity["occurrenceId"].as_str().unwrap()
                ),
                None,
            )
            .await;
            assert_eq!(definition["exact"], true);
            assert_eq!(definition["hash"], *hash);
            assert_eq!(definition["source"], source);
        }
        assert!(done["context"]["instructions"].to_string().contains(marker));
        assert_eq!(done["timeline"][0]["text"], "Lis mon dossier");
        ok(
            &router,
            "PATCH",
            &format!("/api/runs/{id}?workspaceId={workspace}"),
            Some(json!({"name":"Session renommée"})),
        )
        .await;
        assert_eq!(
            ok(
                &router,
                "GET",
                &format!("/api/runs/{id}?workspaceId={workspace}"),
                None
            )
            .await["name"],
            "Session renommée"
        );
    }
    assert_eq!(
        ok(&router, "GET", &format!("/api/runs?workspaceId={a}"), None)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn legacy_database_is_backed_up_migrated_once_and_then_unused() {
    let root = tempfile::tempdir().unwrap();
    tokio::fs::create_dir_all(root.path().join("data"))
        .await
        .unwrap();
    let db = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}?mode=rwc",
        root.path().join("data/zedflow.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("CREATE TABLE compositions(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO compositions VALUES(?,?)")
        .bind("legacy")
        .bind(doc("legacy").to_string())
        .execute(&db)
        .await
        .unwrap();
    db.close().await;
    for dir in ["a", "home"] {
        std::fs::create_dir_all(root.path().join(dir)).unwrap();
    }
    // Startup no longer silently converts definitions: explicit package import
    // replaces the historical startup writer, preserving backup/idempotence.
    let error = support::open_router(
        root.path().join("data"),
        root.path().join("a"),
        vec![],
        root.path().join("home"),
    )
    .await
    .err()
    .expect("legacy import required");
    assert!(
        error.to_string().contains("migrate-compositions"),
        "{error:#}"
    );
    let store = zf_storage::flow_store::FlowStore::new(
        root.path().join("home"),
        std::sync::Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        )),
    );
    let receipt = zf_storage::legacy_compositions::import(
        &root.path().join("data"),
        &root.path().join("a"),
        &root.path().join("home"),
        &store,
        false,
    )
    .await
    .unwrap();
    let (router, service) = app_with_service(root.path()).await;
    let flows = ok(&router, "GET", "/api/flows", None).await;
    assert_eq!(flows.as_array().unwrap().len(), 1);
    assert!(receipt.complete && receipt.backup.as_ref().unwrap().is_file());
    let package = Path::new(flows[0]["path"].as_str().unwrap());
    assert_eq!(package, root.path().join("a/.zedflow/flow/legacy"));
    let path = package.join("flow.rs");
    let source = tokio::fs::read_to_string(&path).await.unwrap();
    let retained: String =
        sqlx::query_scalar("SELECT document FROM compositions WHERE id='legacy'")
            .fetch_one(&service.database())
            .await
            .unwrap();
    assert_eq!(retained, doc("legacy").to_string());
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    let (router, service) = app_with_service(root.path()).await;
    assert_eq!(
        ok(&router, "GET", "/api/flows", None)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), source);
    service.shutdown().await.unwrap();
    drop(router);
    drop(service);
    // Import belongs to workspace A; the database remains usable with another
    // initial workspace and another explicit global catalogue at next startup.
    let other_home = root.path().join("other-home");
    std::fs::create_dir(&other_home).unwrap();
    let (router, service) = support::open_router(
        root.path().join("data"),
        root.path().join("b"),
        vec![],
        other_home,
    )
    .await
    .unwrap();
    assert_eq!(
        ok(&router, "GET", "/api/flows", None)
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), source);
    service.shutdown().await.unwrap();
}
