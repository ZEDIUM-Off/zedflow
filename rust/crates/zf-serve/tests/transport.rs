use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::ServiceExt;
use zf_execution::{
    commands::{Actor, CommandAuthorizer, CommandKind, ExecutionError},
    service::{ExecutionOptions, ExecutionService},
    start::{StartDefinition, StartRequest},
};
use zf_storage::workspaces::Workspace;

#[derive(Default)]
struct Policy {
    calls: Mutex<Vec<CommandKind>>,
    deny_start: bool,
}
#[async_trait::async_trait]
impl CommandAuthorizer for Policy {
    async fn authorize(
        &self,
        actor: &Actor,
        kind: CommandKind,
        workspace: &Workspace,
        _: Option<&Value>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(actor.workspace_id == workspace.id, "scope mismatch");
        self.calls.lock().unwrap().push(kind);
        anyhow::ensure!(
            !(self.deny_start && kind == CommandKind::Start),
            ExecutionError::Forbidden("start refused by fixture policy".into())
        );
        Ok(())
    }
}
struct Fixture {
    _root: tempfile::TempDir,
    service: ExecutionService,
    router: Router,
    policy: Arc<Policy>,
}
impl Fixture {
    async fn new(deny_start: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        let data = root.path().join("data");
        std::fs::create_dir(&workspace).unwrap();
        std::fs::create_dir(&home).unwrap();
        let policy = Arc::new(Policy {
            deny_start,
            ..Default::default()
        });
        let service = ExecutionService::open(ExecutionOptions {
            data: data.clone(),
            workspace: workspace.clone(),
            flow_home: home.clone(),
            context_home: Some(home.clone()),
            skill_dirs: vec![],
            authorizer: policy.clone(),
        })
        .await
        .unwrap();
        let router = zf_serve::server::router_for_service(
            service.clone(),
            data,
            workspace,
            vec![],
            home.clone(),
            Some(home),
            None,
            tokio_util::sync::CancellationToken::new(),
            Arc::new(zf_serve::app_updates::Updates::new(Default::default())),
        )
        .await
        .unwrap();
        Self {
            _root: root,
            service,
            router,
            policy,
        }
    }
    fn actor(&self) -> Actor {
        Actor {
            id: "fixture".into(),
            workspace_id: self.service.default_workspace_id().into(),
        }
    }
    async fn request(&self, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(if value.is_null() {
                Body::empty()
            } else {
                Body::from(value.to_string())
            })
            .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!({"body":String::from_utf8_lossy(&bytes)})),
        )
    }
    async fn idle(&self, id: &str) {
        tokio::time::timeout(Duration::from_secs(10), self.service.wait_idle(id))
            .await
            .unwrap()
            .unwrap();
    }
}
fn composition() -> Value {
    json!({"id":"transport-fixture","name":"Transport fixture","nodes":[{"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},{"id":"set","position":{"x":0,"y":0},"data":{"kind":"set","label":"Set","config":{"field":"result","value":{"custom":{"preserved":true}}}}},{"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}],"edges":[{"id":"a","source":"s","target":"set"},{"id":"b","source":"set","target":"e"}]})
}

#[tokio::test]
async fn http_and_direct_commands_share_admission_and_preserve_open_values() {
    let f = Fixture::new(false).await;
    let (status, ack) = f
        .request(
            "POST",
            "/api/runs",
            json!({"composition":composition(),"input":{"opaque":{"nested":[1,true,"exact"]}}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{ack}");
    let id = ack["id"].as_str().unwrap();
    f.idle(id).await;
    let run = f.service.read(&f.actor(), id).await.unwrap();
    assert_eq!(run["input"]["opaque"], json!({"nested":[1,true,"exact"]}));
    let (status, rename) = f
        .request(
            "PATCH",
            &format!("/api/runs/{id}"),
            json!({"name":"renamed"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rename}");
    assert_eq!(
        f.service.read(&f.actor(), id).await.unwrap()["name"],
        "renamed"
    );
    assert!(f.policy.calls.lock().unwrap().contains(&CommandKind::Start));
    assert!(
        f.policy
            .calls
            .lock()
            .unwrap()
            .contains(&CommandKind::Rename)
    );
    let events: Vec<String> = sqlx::query_scalar("SELECT document FROM events WHERE run=?")
        .bind(id)
        .fetch_all(&f.service.database())
        .await
        .unwrap();
    assert!(events.iter().any(|raw| {
        let value: Value = serde_json::from_str(raw).unwrap();
        value["actor"]["id"] == "local-http"
    }));
    f.service.shutdown().await.unwrap();
}
#[tokio::test]
async fn http_cannot_bypass_service_policy_or_maintenance() {
    let denied = Fixture::new(true).await;
    let (status, body) = denied
        .request("POST", "/api/runs", json!({"composition":composition()}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM runs")
        .fetch_one(&denied.service.database())
        .await
        .unwrap();
    assert_eq!(count, 0);
    denied.service.shutdown().await.unwrap();
    let f = Fixture::new(false).await;
    let guard = f.service.try_begin_maintenance().unwrap();
    let (status, body) = f
        .request("POST", "/api/runs", json!({"composition":composition()}))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    drop(guard);
    f.service.shutdown().await.unwrap();
}
#[tokio::test]
async fn workspace_scope_protects_raw_and_commands_and_raw_preserves_exact_bytes() {
    let f = Fixture::new(false).await;
    let ack = f
        .service
        .start(
            &f.actor(),
            StartRequest {
                definition: StartDefinition::Inline(serde_json::from_value(composition()).unwrap()),
                input: Default::default(),
                model_bindings: json!({}),
                node_path: None,
                prepared_context: None,
                preview_metadata: None,
            },
        )
        .await
        .unwrap();
    let id = ack["id"].as_str().unwrap();
    f.idle(id).await;
    let parts = vec![
        "{ \"opaque\": ".to_string(),
        "[true,1e+02,\"é\\n\"], \"z\": 0 }\n".to_string(),
    ];
    let exact = parts.concat();
    f.service
        .content()
        .put_record(
            id,
            "inference-raw",
            "invocation",
            &zf_runtime::inference_raw::document("fixture", &parts),
        )
        .await
        .unwrap();
    let path = format!("/api/runs/{id}/requests/invocation/raw");
    let response = f
        .router
        .clone()
        .oneshot(Request::builder().uri(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .as_ref(),
        exact.as_bytes()
    );
    let (status, _) = f
        .request("GET", &format!("{path}?workspaceId=foreign"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = f
        .request(
            "PATCH",
            &format!("/api/runs/{id}?workspaceId=foreign"),
            json!({"name":"forbidden"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    f.service.shutdown().await.unwrap();
}
#[tokio::test]
async fn conversion_and_deletion_use_revision_checked_package_commands() {
    let f = Fixture::new(false).await;
    let (status, flow) = f
        .request("POST", "/api/flows", json!({"composition":composition()}))
        .await;
    assert_eq!(status, StatusCode::OK, "{flow}");
    let key = flow["key"].as_str().unwrap();
    let stale = format!("/api/flows/{key}?expectedHash=stale");
    let (status, _) = f.request("DELETE", &stale, Value::Null).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let path = format!(
        "/api/flows/{key}?expectedHash={}",
        flow["hash"].as_str().unwrap()
    );
    let (status, value) = f.request("DELETE", &path, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["deleted"], true);
    f.service.shutdown().await.unwrap();
}
#[tokio::test]
async fn validation_version_and_deferred_exports_return_explicit_responses() {
    let f = Fixture::new(false).await;
    let (status, value) = f.request("POST", "/api/validate", composition()).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["valid"], true);
    let (status, value) = f.request("GET", "/api/version", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(zf_serve::app_updates::valid_id(
        value["daemon"]["buildId"].as_str().unwrap()
    ));
    let (status, value) = f.request("POST", "/api/generate", composition()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        value["error"]
            .as_str()
            .unwrap()
            .contains("export_unavailable")
    );
    let request = Request::builder()
        .method("POST")
        .uri("/api/validate")
        .header("content-type", "application/json")
        .header("x-zedflow-protocol", "999999")
        .body(Body::from(composition().to_string()))
        .unwrap();
    assert_eq!(
        f.router.clone().oneshot(request).await.unwrap().status(),
        StatusCode::CONFLICT
    );
    f.service.shutdown().await.unwrap();
}
#[test]
fn build_identity_never_opens_data_and_retains_launch_flags() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("absent");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zedflow-daemon"))
        .args(["--build-info", "--data"])
        .arg(&data)
        .output()
        .unwrap();
    assert!(result.status.success());
    let identity: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(identity["component"], "daemon");
    assert!(zf_serve::app_updates::valid_id(
        identity["buildId"].as_str().unwrap()
    ));
    assert!(!data.exists());
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_zedflow-daemon"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for flag in [
        "--listen",
        "--data",
        "--workspace",
        "--web",
        "--flow-home",
        "--context-home",
        "--maintain-keep-session",
        "--maintenance-cutoff",
    ] {
        assert!(help.contains(flag), "{flag}");
    }
}

#[tokio::test]
async fn catalogue_conversion_returns_new_package_identity_and_preserves_legacy_bytes() {
    let f = Fixture::new(false).await;
    let document: zf_flows::schema::Composition = serde_json::from_value(composition()).unwrap();
    let source = zf_flows::flow_source::render(
        &document,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    let legacy = f._root.path().join("workspace/.agents/flows");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("legacy.rs"), &source).unwrap();
    let (status, files) = f.request("GET", "/api/flows", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{files}");
    let selected = files
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["id"] == document.id)
        .unwrap();
    let old = selected["key"].clone();
    let (status, result) = f
        .request(
            "POST",
            "/api/flows/convert",
            json!({"key":old,"expectedHash":selected["hash"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["oldKey"], old);
    assert_ne!(result["newKey"], old);
    assert_eq!(result["flow"]["source"], source);
    assert!(result["changedConsumers"].is_array());
    assert!(result.get("key").is_none());
    assert!(!legacy.join("legacy.rs").exists());
    f.service.shutdown().await.unwrap();
}

#[tokio::test]
async fn maintenance_also_closes_catalogue_reads_and_pure_workspace_previews() {
    let f = Fixture::new(false).await;
    let guard = f.service.try_begin_maintenance().unwrap();
    for path in ["/api/runs", "/api/flows", "/api/context-types"] {
        let (status, body) = f.request("GET", path, Value::Null).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}: {body}");
    }
    let (status, body) = f
        .request(
            "POST",
            "/api/type-examples/query",
            json!({"dataType":{"kind":"text"}}),
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        f.request("GET", "/api/health", Value::Null).await.0,
        StatusCode::OK
    );
    drop(guard);
    f.service.shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn daemon_process_serves_assets_then_releases_data_on_sigterm() {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let root = tempfile::tempdir().unwrap();
    for dir in ["workspace", "home", "web"] {
        std::fs::create_dir(root.path().join(dir)).unwrap();
    }
    std::fs::write(root.path().join("web/index.html"), "<p>isolated client</p>").unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_zedflow-daemon"))
        .current_dir(root.path())
        .args([
            "--listen",
            "127.0.0.1:0",
            "--data",
            "data",
            "--workspace",
            "workspace",
            "--flow-home",
            "home",
            "--context-home",
            "home",
            "--web",
            "web",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let endpoint = line.strip_prefix("Zedflow daemon ").unwrap();
    let response = reqwest::get(endpoint).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.text().await.unwrap(), "<p>isolated client</p>");
    assert!(zf_storage::migration::lock(&root.path().join("data")).is_err());
    let status = tokio::process::Command::new("kill")
        .args(["-TERM", &child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    assert!(status.success());
    assert!(
        tokio::time::timeout(Duration::from_secs(20), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    assert!(zf_storage::migration::lock(&root.path().join("data")).is_ok());
}

#[tokio::test]
async fn dropping_a_convenience_router_drains_its_service_and_releases_ownership() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    let data = root.path().join("data");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir(&workspace).unwrap();
    let router = zf_serve::server::router_with_home(data.clone(), workspace, vec![], home)
        .await
        .unwrap();
    assert!(zf_storage::migration::lock(&data).is_err());
    drop(router);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(lock) = zf_storage::migration::lock(&data) {
                drop(lock);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("last router releases service ownership");
}
