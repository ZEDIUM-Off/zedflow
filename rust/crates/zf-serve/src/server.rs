//! Persistence and public commands. No graph scheduling lives in this module.
use crate::api::AuthoringJson;
use zf_execution::preparation::RuntimeSelection;
use zf_execution::{
    commands::{Actor, CommandAuthorizer, CommandKind},
    service::{ExecutionOptions, ExecutionService},
};
use zf_flows::schema::Composition;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::{
    content_store::ContentStore,
    flow_store::{FlowFile, FlowStore},
    session_sync::SessionSync,
    workspaces::{self, Workspace},
};

use adk_graph::prelude::*;
use anyhow::Context;
use axum::{
    Json, Router,
    extract::{Path, Query, State as App},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, patch, post},
};

use serde::Deserialize;
use sqlx::SqlitePool;

use std::{convert::Infallible, path::PathBuf, sync::Arc, time::Duration};

use uuid::Uuid;
#[derive(Clone)]
struct Backend {
    _lifetime: Arc<RouterLifetime>,
    updates: Arc<crate::app_updates::Updates>,
    service: ExecutionService,
    db: SqlitePool,
    content: ContentStore,
    sync: SessionSync,
    daemon: Value,
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    default_workspace_id: String,
    flows: FlowStore,
    home: PathBuf,
    context_home: Option<PathBuf>,
}
struct RouterLifetime(tokio_util::sync::CancellationToken);
impl Drop for RouterLifetime {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
pub struct LocalAuthorizer;
#[async_trait::async_trait]
impl CommandAuthorizer for LocalAuthorizer {
    async fn authorize(
        &self,
        actor: &Actor,
        _command: CommandKind,
        workspace: &Workspace,
        _run: Option<&Value>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(actor.workspace_id == workspace.id, "Workspace mismatch");
        Ok(())
    }
}
impl Backend {
    fn actor(&self, workspace: Option<&str>) -> Actor {
        Actor {
            id: "local-http".into(),
            workspace_id: workspace.unwrap_or(&self.default_workspace_id).into(),
        }
    }
    async fn run_actor(&self, id: &str) -> anyhow::Result<Actor> {
        let (workspace, _) = self.sync.head(id).await?;
        Ok(self.actor(Some(workspace.as_str().context("Run workspace absent")?)))
    }
}
#[derive(Debug)]
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        let error = e.into();
        let status = error_status(&error);
        Self(status, format!("{error:#}"))
    }
}
type Api<T> = std::result::Result<Json<T>, ApiError>;
pub async fn router(data: PathBuf, workspace: PathBuf) -> anyhow::Result<Router> {
    router_with_skills(data, workspace, vec![]).await
}
pub async fn router_with_skills(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
) -> anyhow::Result<Router> {
    router_with_source_homes(data, workspace, skill_dirs, None, None).await
}
/// Explicit global root keeps fixtures and secondary daemon instances isolated.
pub async fn router_with_home(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    home: PathBuf,
) -> anyhow::Result<Router> {
    router_with_source_homes(data, workspace, skill_dirs, Some(home.clone()), Some(home)).await
}
/// Flow catalogs and runtime context may use independently isolated global roots.
pub async fn router_with_source_homes(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    flow_home: Option<PathBuf>,
    context_home: Option<PathBuf>,
) -> anyhow::Result<Router> {
    router_with_runtime(
        data,
        workspace,
        skill_dirs,
        flow_home,
        context_home,
        None,
        tokio_util::sync::CancellationToken::new(),
    )
    .await
}
/// The process supplies its listening endpoint and cancellation for persistent streams.
pub async fn router_with_runtime(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    flow_home: Option<PathBuf>,
    context_home: Option<PathBuf>,
    endpoint: Option<String>,
    shutdown: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Router> {
    router_with_runtime_and_updates(
        data,
        workspace,
        skill_dirs,
        flow_home,
        context_home,
        endpoint,
        shutdown,
        Arc::new(crate::app_updates::Updates::new(
            crate::app_updates::Config::default(),
        )),
    )
    .await
}
#[allow(clippy::too_many_arguments)]
pub async fn router_with_runtime_and_updates(
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    flow_home: Option<PathBuf>,
    context_home: Option<PathBuf>,
    endpoint: Option<String>,
    shutdown: tokio_util::sync::CancellationToken,
    updates: Arc<crate::app_updates::Updates>,
) -> anyhow::Result<Router> {
    let home = flow_home
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .context("Flow home is required")?;
    let home = tokio::fs::canonicalize(home)
        .await
        .context("Resolve flow home")?;
    let service = ExecutionService::open(ExecutionOptions {
        data: data.clone(),
        workspace: workspace.clone(),
        flow_home: home.clone(),
        context_home: context_home.clone(),
        skill_dirs: skill_dirs.clone(),
        authorizer: Arc::new(LocalAuthorizer),
    })
    .await?;
    let router = router_for_service(
        service.clone(),
        data,
        workspace,
        skill_dirs,
        home,
        context_home,
        endpoint,
        shutdown,
        updates,
    )
    .await?;
    let stop = service.sync().shutdown.clone();
    tokio::spawn(async move {
        stop.cancelled().await;
        let _ = service.shutdown().await;
    });
    Ok(router)
}
#[allow(clippy::too_many_arguments)]
pub async fn router_for_service(
    service: ExecutionService,
    data: PathBuf,
    workspace: PathBuf,
    skill_dirs: Vec<PathBuf>,
    home: PathBuf,
    context_home: Option<PathBuf>,
    endpoint: Option<String>,
    shutdown: tokio_util::sync::CancellationToken,
    updates: Arc<crate::app_updates::Updates>,
) -> anyhow::Result<Router> {
    let data = tokio::fs::canonicalize(data).await?;
    let workspace = tokio::fs::canonicalize(workspace).await?;
    let db = service.database();
    let content = service.content();
    let sync = service.sync();
    let default_workspace_id = service.default_workspace_id().to_owned();
    let flows = FlowStore::new(
        home.clone(),
        Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        )),
    );
    sqlx::query("CREATE TABLE IF NOT EXISTS daemon_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
        .execute(&db)
        .await?;
    sqlx::query("INSERT OR IGNORE INTO daemon_meta(key,value) VALUES('id',?)")
        .bind(Uuid::new_v4().to_string())
        .execute(&db)
        .await?;
    let daemon_id: String = sqlx::query_scalar("SELECT value FROM daemon_meta WHERE key='id'")
        .fetch_one(&db)
        .await?;
    let host = tokio::fs::read_to_string("/proc/sys/kernel/hostname")
        .await
        .unwrap_or_else(|_| "local".into());
    let daemon = json!({"id":daemon_id,"instanceId":Uuid::new_v4().to_string(),"host":host.trim(),"version":crate::app_updates::build()["version"],"build":crate::app_updates::build(),"endpoint":endpoint});
    let rtc = crate::rtc::router(sync.clone())?;
    let authoring = crate::api::router(service.clone(), flows.clone());
    let scope = (service.clone(), db.clone(), default_workspace_id.clone());
    let sync_shutdown = sync.shutdown.clone();
    tokio::spawn(async move {
        tokio::select! {_=shutdown.cancelled()=>{sync_shutdown.cancel();},_=sync_shutdown.cancelled()=>{}}
    });
    let backend = Backend {
        _lifetime: Arc::new(RouterLifetime(sync.shutdown.clone())),
        updates: updates.clone(),
        service,
        db,
        content,
        sync,
        daemon,
        data,
        workspace,
        skill_dirs,
        default_workspace_id,
        flows,
        home,
        context_home,
    };
    for workspace in workspaces::list(&backend.db).await? {
        if workspace.open {
            ensure_defaults(&backend, &workspace).await?;
        }
    }
    Ok(Router::new()
        .route("/api/health", get(health))
        .route("/api/version", get(app_version))
        .route("/api/updates/apply", post(app_update))
        .route(
            "/api/capabilities",
            get(|| async { Json(zf_runtime::capabilities::catalog()) }),
        )
        .route("/api/auth/codex", get(codex_status))
        .route("/api/models", get(models))
        .route("/api/context", get(workspace_context))
        .route("/api/workspaces", get(list_workspaces).post(open_workspace))
        .route("/api/workspaces/{id}", patch(update_workspace))
        .route("/api/filesystem", get(browse_filesystem))
        .route("/api/sessions/export", post(export_sessions))
        .route("/api/sessions/import", post(import_sessions))
        .route("/api/sessions/exports/{id}", get(download_sessions))
        .route("/api/flows", get(list_flows).post(store_flow))
        .route("/api/flows/convert", post(convert_flow))
        .route(
            "/api/examples/working-system",
            post(install_working_system_example),
        )
        .route("/api/flows/{key}", get(get_flow).delete(delete_flow))
        .route("/api/compositions", get(compositions).post(store))
        .route("/api/compositions/{id}", get(composition))
        .route("/api/validate", post(validate))
        .route("/api/generate", post(generate))
        .route("/api/build", post(build))
        .route("/api/runtime-graphs/prepare", post(prepare_runtime))
        .route("/api/runs", get(runs).post(start))
        .route("/api/runs/{id}", get(run).patch(rename_run))
        .route("/api/runs/{id}/snapshot", get(snapshot))
        .route(
            "/api/runs/{id}/activities/{occurrence}",
            get(activity_detail),
        )
        .route(
            "/api/runs/{id}/boundaries/{occurrence}",
            get(passage_boundary),
        )
        .route("/api/runs/{id}/tools/{call}", get(tool_detail))
        .route("/api/runs/{id}/tools/{call}/output", get(tool_output))
        .route("/api/runs/{id}/context/{invocation}", get(context_detail))
        .route("/api/runs/{id}/requests/{invocation}", get(request_detail))
        .route("/api/runs/{id}/requests/{invocation}/raw", get(request_raw))
        .route("/api/runs/{id}/context-windows", get(context_windows))
        .route("/api/runs/{id}/context-program", get(context_program))
        .route(
            "/api/runs/{id}/context-window",
            get(context_window).patch(patch_context_window),
        )
        .route(
            "/api/runs/{id}/context-window/select",
            post(select_context_window),
        )
        .route("/api/runs/{id}/state", get(state_detail))
        .route("/api/runs/{id}/metrics", get(session_metrics))
        .route("/api/runs/{id}/event-history", get(event_history))
        .route("/api/runs/{id}/timeline", get(timeline_history))
        .route("/api/runs/{id}/flow-source", get(executed_source))
        .route("/api/runs/{id}/definition", get(executed_definition))
        .route("/api/runs/{id}/revisions", get(run_revisions))
        .route("/api/runs/{id}/preview-source", get(preview_source))
        .route("/api/runs/preview", post(preview_run))
        .route("/api/runs/{id}/answer", post(answer))
        .route("/api/runs/{id}/models", patch(select_model))
        .route("/api/runs/{id}/capabilities", post(activate_capability))
        .route("/api/runs/{id}/messages", post(queue_message))
        .route("/api/runs/{id}/messages/{message}", delete(remove_message))
        .route("/api/runs/{id}/abort", post(abort_run))
        .route("/api/runs/{id}/resume", post(resume_run))
        .route("/api/runs/{id}/events", get(events))
        .with_state(backend)
        .merge(rtc)
        .merge(authoring)
        .layer(middleware::from_fn_with_state(scope, workspace_scope))
        .layer(middleware::from_fn_with_state(updates, version_gate)))
}

async fn workspace_scope(
    App((service, db, default_workspace_id)): App<(ExecutionService, SqlitePool, String)>,
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let Query(query) = match Query::<WorkspaceQuery>::try_from_uri(request.uri()) {
        Ok(query) => query,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error":error.to_string()})),
            )
                .into_response();
        }
    };
    let workspace_id = query.workspace_id.unwrap_or(default_workspace_id);
    if let Some(tail) = request.uri().path().strip_prefix("/api/runs/")
        && tail != "preview"
    {
        let id = tail.split('/').next().unwrap_or_default();
        let owner: std::result::Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
            "SELECT json_extract(document,'$.workspaceId') FROM runs WHERE id=?",
        )
        .bind(id)
        .fetch_optional(&db)
        .await;
        match owner {
            Ok(Some(owner)) if owner == workspace_id => {}
            Ok(_) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error":"Session absente de ce workspace"})),
                )
                    .into_response();
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error":error.to_string()})),
                )
                    .into_response();
            }
        }
    }
    let _read = if matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && request.uri().path().starts_with("/api/")
        && !matches!(request.uri().path(), "/api/health" | "/api/version")
    {
        let actor = Actor {
            id: "local-http".into(),
            workspace_id,
        };
        let id = request
            .uri()
            .path()
            .strip_prefix("/api/runs/")
            .map(|tail| tail.split('/').next().unwrap_or_default());
        match service.read_scope(&actor, id).await {
            Ok(guard) => Some(guard),
            Err(error) => return ApiError::from(error).into_response(),
        }
    } else {
        None
    };
    next.run(request).await
}

async fn convert_flow(
    App(b): App<Backend>,
    AuthoringJson(value): AuthoringJson<Value>,
) -> Api<Value> {
    if value.get("key").is_some() {
        let actor = b.actor(value["workspaceId"].as_str());
        let request = zf_execution::authoring::ConvertFlow {
            key: value["key"].as_str().context("Flow key absent")?.into(),
            expected_hash: value["expectedHash"]
                .as_str()
                .context("Expected hash absent")?
                .into(),
        };
        return Ok(Json(serde_json::to_value(
            b.service.convert_flow(&actor, request).await?,
        )?));
    }
    Ok(Json(serde_json::to_value(
        zf_flows::flow_source::convert_v1(
            &serde_json::from_value(value)?,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )?,
    )?))
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceQuery {
    workspace_id: Option<String>,
}

async fn selected_workspace(b: &Backend, id: Option<&str>) -> anyhow::Result<Workspace> {
    workspaces::get(&b.db, id.unwrap_or(&b.default_workspace_id)).await
}

async fn models(App(b): App<Backend>, Query(query): Query<WorkspaceQuery>) -> Api<Value> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    let docs = b
        .flows
        .list(&workspace)
        .await?
        .into_iter()
        .filter_map(|flow| flow.composition)
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(Json(zf_runtime::model_catalog::catalog(&docs).await))
}
async fn workspace_context(
    App(b): App<Backend>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Value> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::to_value(
        ContextSnapshot::load_with_home(&workspace.path, &b.skill_dirs, b.context_home.as_deref())
            .await?,
    )?))
}
async fn codex_status() -> Api<Value> {
    Ok(Json(zf_runtime::codex::status().await?))
}
async fn health(App(b): App<Backend>) -> Json<Value> {
    Json(
        json!({"name":"Zedflow","daemon":b.daemon,"adk":"2.2.0","defaultWorkspaceId":b.default_workspace_id,"workspace":{"id":b.default_workspace_id,"path":b.workspace,"host":b.daemon["host"]},"capabilities":["start","end","set","context","model","agent","tool","condition","input","output","subgraph"]}),
    )
}
async fn list_workspaces(App(b): App<Backend>) -> Api<Vec<Workspace>> {
    Ok(Json(workspaces::list(&b.db).await?))
}
#[derive(Deserialize)]
struct OpenWorkspace {
    path: PathBuf,
}
async fn open_workspace(
    App(b): App<Backend>,
    Json(request): Json<OpenWorkspace>,
) -> Api<Workspace> {
    let workspace = b
        .service
        .open_workspace(&b.actor(None), &request.path)
        .await?;
    ensure_defaults(&b, &workspace).await?;
    Ok(Json(workspace))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkingSystemExample {
    workspace_id: Option<String>,
    working_directory: String,
}
async fn install_working_system_example(
    App(b): App<Backend>,
    Json(request): Json<WorkingSystemExample>,
) -> Api<Value> {
    let workspace = workspaces::get(
        &b.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&b.default_workspace_id),
    )
    .await?;
    Ok(Json(
        install_example(&b, &workspace, &request.working_directory).await?,
    ))
}
#[derive(Deserialize)]
struct WorkspaceUpdate {
    name: Option<String>,
    open: Option<bool>,
}
async fn update_workspace(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<WorkspaceUpdate>,
) -> Api<Workspace> {
    Ok(Json(
        b.service
            .update_workspace(
                &b.actor(Some(&id)),
                zf_execution::administration::WorkspaceUpdate {
                    name: request.name,
                    open: request.open,
                },
            )
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Browse {
    path: Option<PathBuf>,
    #[serde(default)]
    show_hidden: bool,
}
async fn browse_filesystem(App(b): App<Backend>, Query(query): Query<Browse>) -> Api<Value> {
    let requested = query
        .path
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(&b.home);
    let path = if requested == std::path::Path::new("~") {
        b.home.clone()
    } else if let Ok(relative) = requested.strip_prefix("~") {
        b.home.join(relative)
    } else {
        requested.to_owned()
    };
    Ok(Json(
        workspaces::browse(&path, &b.home, query.show_hidden).await?,
    ))
}
async fn list_flows(
    App(b): App<Backend>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<FlowFile>> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    recover_definition_writes(&b, &workspace).await?;
    Ok(Json(b.flows.list(&workspace).await?))
}
async fn get_flow(
    App(b): App<Backend>,
    Path(key): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<FlowFile> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    recover_definition_writes(&b, &workspace).await?;
    Ok(Json(b.flows.get(&workspace, &key).await?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreFlow {
    workspace_id: Option<String>,
    composition: Composition,
    scope: Option<String>,
    key: Option<String>,
    expected_hash: Option<String>,
}
async fn store_flow(
    App(b): App<Backend>,
    AuthoringJson(request): AuthoringJson<StoreFlow>,
) -> Api<FlowFile> {
    Ok(Json(
        b.service
            .store_flow(
                &b.actor(request.workspace_id.as_deref()),
                zf_execution::authoring::StoreFlow {
                    composition: request.composition,
                    scope: request.scope,
                    key: request.key,
                    expected_hash: request.expected_hash,
                },
            )
            .await?,
    ))
}
async fn recover_definition_writes(b: &Backend, workspace: &Workspace) -> anyhow::Result<()> {
    b.service
        .recover_catalog(&b.actor(Some(&workspace.id)))
        .await
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteFlow {
    workspace_id: Option<String>,
    expected_hash: String,
}
async fn delete_flow(
    App(b): App<Backend>,
    Path(key): Path<String>,
    Query(request): Query<DeleteFlow>,
) -> Api<Value> {
    b.service
        .delete_flow_package(
            &b.actor(request.workspace_id.as_deref()),
            zf_execution::authoring::DeleteFlowPackage {
                key,
                expected_hash: request.expected_hash,
            },
        )
        .await?;
    Ok(Json(json!({"deleted":true})))
}
async fn compositions(
    App(b): App<Backend>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<Composition>> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    Ok(Json(
        b.flows
            .list(&workspace)
            .await?
            .into_iter()
            .filter_map(|flow| flow.composition)
            .collect(),
    ))
}
async fn composition(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Composition> {
    let workspace = selected_workspace(&b, query.workspace_id.as_deref()).await?;
    let matches: Vec<_> = b
        .flows
        .list(&workspace)
        .await?
        .into_iter()
        .filter(|flow| flow.id == id)
        .collect();
    if matches.len() != 1 {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Flow absent ou ambigu ; utilisez son identité de fichier".into(),
        ));
    }
    Ok(Json(
        matches
            .into_iter()
            .next()
            .and_then(|flow| flow.composition)
            .ok_or_else(|| anyhow::anyhow!("Flow invalide"))?,
    ))
}
async fn store(
    App(b): App<Backend>,
    AuthoringJson(doc): AuthoringJson<Composition>,
) -> Api<Composition> {
    let workspace = selected_workspace(&b, None).await?;
    let exists = b
        .flows
        .list(&workspace)
        .await?
        .into_iter()
        .any(|flow| flow.id == doc.id && flow.scope == "workspace");
    // Legacy clients do not carry a file key or a loaded source hash. They can
    // create a flow, but updating one must use the conflict-aware file API.
    if exists || doc.revision != 0 {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Pour modifier ce flow, utilisez /api/flows avec sa clé de fichier et son hash chargé."
                .into(),
        ));
    }
    let saved = b
        .service
        .store_flow(
            &b.actor(Some(&workspace.id)),
            zf_execution::authoring::StoreFlow {
                composition: doc,
                scope: None,
                key: None,
                expected_hash: None,
            },
        )
        .await?;
    Ok(Json(saved.composition.ok_or_else(|| {
        anyhow::anyhow!("Flow invalide après sauvegarde")
    })?))
}
async fn validate(AuthoringJson(doc): AuthoringJson<Composition>) -> Api<Value> {
    zf_compiler::graph_compiler::validate(&doc, &zf_runtime::materialize::RuntimePrimitives)?;
    Ok(Json(json!({"valid":true,"adk":"2.2.0"})))
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlowSelection {
    workspace_id: Option<String>,
    flow_key: Option<String>,
    flow_hash: Option<String>,
    composition: Option<Composition>,
}
async fn generated_files(b: &Backend, value: Value) -> anyhow::Result<Value> {
    use base64::Engine;
    use zf_execution::cargo_export::ExportRequest;
    let actor = b.actor(value["workspaceId"].as_str());
    let request = if let Some(id) = value["runId"].as_str() {
        ExportRequest::Passage {
            run_id: id.into(),
            query: zf_runtime::inspection::DefinitionQuery {
                node_path: value["nodePath"].as_str().map(str::to_owned),
                occurrence_id: value["occurrenceId"].as_str().map(str::to_owned),
                hash: value["hash"].as_str().map(str::to_owned),
            },
        }
    } else if let Some(selection) = value.get("runtimeSelection") {
        ExportRequest::Runtime(serde_json::from_value(selection.clone())?)
    } else if value.get("nodes").is_some() {
        ExportRequest::Draft(serde_json::from_value(value)?)
    } else {
        let selection: FlowSelection = serde_json::from_value(value)?;
        match (
            selection.composition,
            selection.flow_key,
            selection.flow_hash,
        ) {
            (Some(doc), None, None) => ExportRequest::Draft(doc),
            (None, Some(key), Some(expected_hash)) => ExportRequest::Stored { key, expected_hash },
            _ => anyhow::bail!("Choisissez une définition ou une clé de flow avec son hash"),
        }
    };
    let captured = b.service.cargo_export(&actor, request).await?;
    let files:Vec<Value>=captured.project.files.into_iter().map(|(path,bytes)|match String::from_utf8(bytes) {
        Ok(content)=>json!({"path":path,"content":content}),
        Err(error)=>json!({"path":path,"encoding":"base64","content":base64::engine::general_purpose::STANDARD.encode(error.into_bytes())}),
    }).collect();
    let mut result = json!({"files":files,"revision":captured.project.revision});
    if let Some(revision) = captured.execution_revision {
        result["executionRevision"] = revision;
    }
    Ok(result)
}
async fn generate(App(b): App<Backend>, AuthoringJson(value): AuthoringJson<Value>) -> Api<Value> {
    Ok(Json(generated_files(&b, value).await?))
}
async fn build(App(b): App<Backend>, AuthoringJson(value): AuthoringJson<Value>) -> Api<Value> {
    let generated = generated_files(&b, value).await?;
    let dir = b.data.join("builds").join(Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(dir.join("src")).await?;
    for file in generated["files"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Fichiers absents"))?
    {
        let destination = dir.join(file["path"].as_str().unwrap_or_default());
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = file["content"]
            .as_str()
            .context("Contenu d’export absent")?;
        let bytes = if file["encoding"] == "base64" {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(content)?
        } else {
            content.as_bytes().to_vec()
        };
        tokio::fs::write(destination, bytes).await?;
    }
    let mut command = tokio::process::Command::new("cargo");
    command
        .args(["check", "--locked", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "/tmp/zedflow-adk-target".into()),
        )
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(300), command.output()).await??;
    Ok(Json(
        json!({"success":output.status.success(),"output":String::from_utf8_lossy(&output.stderr),"directory":dir,"files":generated["files"]}),
    ))
}
async fn runs(App(b): App<Backend>, Query(query): Query<WorkspaceQuery>) -> Api<Vec<Value>> {
    let rows:Vec<String>=sqlx::query_scalar("SELECT document FROM runs WHERE json_extract(document,'$.workspaceId')=? ORDER BY json_extract(document,'$.updatedAt') DESC")
        .bind(query.workspace_id.as_deref().unwrap_or(&b.default_workspace_id)).fetch_all(&b.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|raw| {
                serde_json::from_str::<Value>(&raw)
                    .map(|run| zf_storage::session_store::summary(&run))
            })
            .collect::<std::result::Result<_, _>>()?,
    ))
}
async fn run(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    Ok(Json(b.service.read(&b.run_actor(&id).await?, &id).await?))
}

async fn activity_detail(
    App(b): App<Backend>,
    Path((id, occurrence)): Path<(String, String)>,
) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let activity = run["activities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["occurrenceId"] == occurrence)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Passage absent".into()))?;
    Ok(Json(
        zf_storage::session_store::hydrate_activity(&b.content, activity).await?,
    ))
}
async fn passage_boundary(
    App(b): App<Backend>,
    Path((id, occurrence)): Path<(String, String)>,
) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let activity = run["activities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|activity| activity["occurrenceId"] == occurrence)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Passage absent".into()))?;
    let state = if let Some(reference) = activity["stateRef"].as_str() {
        Some(b.content.resolve(reference).await?)
    } else {
        None
    };
    Ok(Json(
        json!({"nodePath":activity["path"],"occurrenceId":occurrence,
        "stateRef":activity["stateRef"],"state":state,"flowRevision":activity["flowRevision"]}),
    ))
}
async fn tool_detail(App(b): App<Backend>, Path((id, call)): Path<(String, String)>) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let tool = run["toolActivities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["callId"] == call)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Appel absent".into()))?;
    Ok(Json(
        zf_storage::session_store::hydrate_tool(&b.content, tool).await?,
    ))
}
async fn tool_output(
    App(b): App<Backend>,
    Path((id, call)): Path<(String, String)>,
) -> std::result::Result<axum::response::Response, ApiError> {
    let (run, _) = b.sync.latest(&id).await?;
    let tool = run["toolActivities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["callId"] == call)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Appel absent".into()))?;
    let tool = zf_storage::session_store::hydrate_tool(&b.content, tool).await?;
    let reference = tool["fullOutputRef"]
        .as_str()
        .or_else(|| tool["result"]["fullOutputRef"].as_str())
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                "Sortie intégrale absente de cette session".into(),
            )
        })?;
    let output = b.content.resolve(reference).await?;
    let bytes = zf_storage::content_store::decode_full_output(&output)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=tool-output.bin",
            ),
        ],
        bytes,
    )
        .into_response())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowQuery {
    node_path: String,
    alias: String,
    revision: Option<zf_core::identity::Revision>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextProgramQuery {
    node_path: String,
    hash: Option<String>,
}
async fn context_program(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<ContextProgramQuery>,
) -> Api<Value> {
    let run = definition_run(&b, &id).await?;
    let accepts = |value: &Value| {
        value.is_object()
            && query.hash.as_ref().is_none_or(|hash| {
                value["hash"] == *hash
                    || serde_json::from_value::<zf_context::resources::ContextProgram>(
                        value.clone(),
                    )
                    .ok()
                    .and_then(|program| program.revision().ok())
                    .is_some_and(|revision| revision == *hash)
            })
    };
    let selected = zf_runtime::inspection::definition(
        &b.content,
        &run,
        zf_runtime::inspection::DefinitionQuery {
            node_path: Some(query.node_path.clone()),
            ..Default::default()
        },
    )
    .await?;
    if selected["exact"] == true {
        let instance = selected["instance"].as_str().unwrap_or_default();
        let relative = if instance.is_empty() {
            query.node_path.as_str()
        } else {
            query
                .node_path
                .strip_prefix(&format!("{instance}/"))
                .unwrap_or(&query.node_path)
        };
        if let Some(program) = node_context_config(&selected["composition"], relative)
            .and_then(|config| config.get("contextProgram"))
            .filter(|program| accepts(program))
        {
            return Ok(Json(program.clone()));
        }
    }
    for record in b
        .content
        .records_of_kind(&id, "capability-snapshots")
        .await?
    {
        let snapshot = b.content.resolve(&record.value_ref).await?;
        let program = &snapshot["prepared"]["program"];
        if snapshot["agentPath"] == query.node_path && accepts(program) {
            return Ok(Json(program.clone()));
        }
    }
    Err(ApiError(
        StatusCode::NOT_FOUND,
        "Programme figé absent pour cet agent et cette révision".into(),
    ))
}

async fn context_windows(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    Ok(Json(
        b.service
            .context_windows(&b.run_actor(&id).await?, &id)
            .await?,
    ))
}
async fn context_window(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<WindowQuery>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .context_window(
                &b.run_actor(&id).await?,
                &id,
                zf_execution::administration::WindowQuery {
                    node_path: query.node_path,
                    alias: query.alias,
                    revision: query.revision,
                },
            )
            .await?,
    ))
}

async fn patch_context_window(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(command): Json<zf_execution::administration::WindowEdit>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .patch_window(&b.run_actor(&id).await?, &id, command)
            .await?,
    ))
}
async fn select_context_window(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(command): Json<zf_context::window_preparation::WindowSelectionCommand>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .select_window(&b.run_actor(&id).await?, &id, command)
            .await?,
    ))
}
async fn request_detail(
    App(b): App<Backend>,
    Path((id, invocation)): Path<(String, String)>,
) -> Api<Value> {
    let _ = b.sync.latest(&id).await?;
    let manifest = b
        .content
        .record(&id, "model-requests", &invocation)
        .await?
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Invocation absente".into()))?;
    let capture = b.content.record(&id, "inference-raw", &invocation).await?;
    let dispatched = b
        .content
        .record(&id, "inference-dispatch", &invocation)
        .await?
        .is_some();
    Ok(Json(
        json!({"invocationId":invocation,"manifest":manifest,"capture":capture.as_ref().map(|value| json!({"boundary":value["boundary"],"byteLength":value["byteLength"],"sha256":value["sha256"]})),"status":if dispatched {"sent"} else if capture.is_some(){"prepared"}else{"unavailable"}}),
    ))
}
async fn request_raw(
    App(b): App<Backend>,
    Path((id, invocation)): Path<(String, String)>,
) -> std::result::Result<axum::response::Response, ApiError> {
    use axum::response::IntoResponse;
    let _ = b.sync.latest(&id).await?;
    let capture = b
        .content
        .record(&id, "inference-raw", &invocation)
        .await?
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                "Aucune capture brute pour cette invocation historique".into(),
            )
        })?;
    let bytes = zf_runtime::inference_raw::bytes(&capture)?;
    Ok((
        [
            (
                axum::http::header::CONTENT_TYPE,
                "application/json; charset=utf-8",
            ),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=\"inference-request.json\"",
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn context_detail(
    App(b): App<Backend>,
    Path((id, invocation)): Path<(String, String)>,
) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let snapshot = run["contextSnapshots"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["invocationId"] == invocation)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Invocation absente".into()))?;
    Ok(Json(
        if let Some(reference) = snapshot["contentRef"].as_str() {
            b.content.resolve(reference).await?
        } else {
            snapshot.clone()
        },
    ))
}
async fn session_metrics(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    Ok(Json(b.sync.metrics(&id).await))
}
async fn state_detail(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    let (run, revision) = b.sync.latest(&id).await?;
    let state = if let Some(reference) = run["stateRef"].as_str() {
        b.content.resolve(reference).await?
    } else {
        run["state"].clone()
    };
    Ok(Json(
        json!({"state":state,"checkpoint":run["checkpoint"],"revision":revision}),
    ))
}
async fn executed_source(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let source = if let Some(reference) = run["flowSourceRef"].as_str() {
        b.content.resolve(reference).await?
    } else {
        run["flowSource"].clone()
    };
    Ok(Json(
        json!({"source":source,"hash":run.get("executedSourceHash").unwrap_or(&run["flowRef"]["hash"])}),
    ))
}
async fn executed_definition(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<zf_runtime::inspection::DefinitionQuery>,
) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    Ok(Json(
        zf_runtime::inspection::definition(&b.content, &run, query).await?,
    ))
}
async fn run_revisions(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    let raw: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
        .bind(&id)
        .fetch_one(&b.db)
        .await?;
    Ok(Json(
        zf_runtime::inspection::revisions(&b.content, &serde_json::from_str(&raw)?).await?,
    ))
}
async fn preview_source(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let preview = &run["preview"];
    let composition = if let Some(reference) = preview["sourceCompositionRef"].as_str() {
        b.content.resolve(reference).await?
    } else if preview["sourceComposition"].is_object() {
        preview["sourceComposition"].clone()
    } else {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Cette exécution n’est pas un test de brouillon".into(),
        ));
    };
    Ok(Json(
        json!({"composition":composition,"sourceWorkspaceId":preview["sourceWorkspaceId"],"sourceWorkspacePath":preview["sourceWorkspacePath"]}),
    ))
}
async fn event_history(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<Cursor>,
) -> Api<Value> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT seq,document FROM events WHERE run=? AND seq>? ORDER BY seq LIMIT 101",
    )
    .bind(&id)
    .bind(query.after.unwrap_or(0).max(0))
    .fetch_all(&b.db)
    .await?;
    let more = rows.len() > 100;
    let mut events = Vec::new();
    let mut cursor = query.after.unwrap_or(0);
    for (seq, raw) in rows.into_iter().take(100) {
        let event: Value = serde_json::from_str(&raw)?;
        events.push(json!({"seq":seq,"event":zf_storage::session_store::hydrate_event(&b.content,&event).await?}));
        cursor = seq;
    }
    Ok(Json(
        json!({"events":events,"cursor":cursor,"hasMore":more}),
    ))
}
#[derive(Deserialize)]
struct TimelineCursor {
    before: Option<i64>,
}
async fn timeline_history(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<TimelineCursor>,
) -> Api<Value> {
    let (run, _) = b.sync.latest(&id).await?;
    let entries: Vec<_> = run["timeline"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| {
            query
                .before
                .is_none_or(|before| e["seq"].as_i64().unwrap_or(0) < before)
        })
        .collect();
    let entries: Vec<_> = entries.into_iter().cloned().collect();
    let (entries, more, before) = zf_storage::session_sync::timeline_page(&entries);
    Ok(Json(
        json!({"entries":entries,"before":before,"hasMore":more}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExportSessions {
    workspace_id: String,
    session_ids: Vec<String>,
}

async fn export_sessions(
    App(b): App<Backend>,
    Json(request): Json<ExportSessions>,
) -> Api<zf_storage::session_archive::ExportResponse> {
    Ok(Json(
        b.service
            .export_sessions(&b.actor(Some(&request.workspace_id)), &request.session_ids)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportSessions {
    workspace_id: String,
    path: PathBuf,
}

async fn import_sessions(
    App(b): App<Backend>,
    Json(request): Json<ImportSessions>,
) -> Api<zf_storage::session_archive::ImportResponse> {
    Ok(Json(
        b.service
            .import_sessions(&b.actor(Some(&request.workspace_id)), &request.path)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadWorkspace {
    workspace_id: String,
}

async fn download_sessions(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(query): Query<DownloadWorkspace>,
) -> std::result::Result<impl IntoResponse, ApiError> {
    let workspace = workspaces::get(&b.db, &query.workspace_id).await?;
    let id = id
        .strip_suffix(".zip")
        .ok_or_else(|| anyhow::anyhow!("Archive ZIP requise"))?;
    let bytes = b
        .service
        .download_sessions(&b.actor(Some(&workspace.id)), id)
        .await
        .map_err(|error| {
            let status = error_status(&error);
            if let Some(domain) = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<zf_execution::commands::ExecutionError>())
            {
                return ApiError(status, domain.to_string());
            }
            let cause = if error.chain().any(|cause| cause.is::<std::io::Error>()) {
                "io"
            } else if error.chain().any(|cause| cause.is::<serde_json::Error>()) {
                "metadata_json"
            } else {
                "validation"
            };
            // Log only a technical category, never paths, metadata or archive bytes.
            eprintln!(
                "{}",
                json!({"event":"session_download_failed","status":status.as_u16(),"cause":cause})
            );
            ApiError(status, "Téléchargement de session impossible".into())
        })?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"zedflow-sessions-{id}.zip\""),
            ),
        ],
        bytes,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Start {
    #[serde(default)]
    runtime_selection: Option<RuntimeSelection>,
    #[serde(flatten)]
    flow: FlowSelection,
    #[serde(default)]
    input: adk_graph::State,
    #[serde(default = "empty_bindings")]
    model_bindings: Value,
    #[serde(default)]
    node_path: Option<String>,
    #[serde(skip)]
    prepared_context: Option<ContextSnapshot>,
    #[serde(skip)]
    preview_metadata: Option<Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewRun {
    workspace_id: String,
    composition: Composition,
    #[serde(default)]
    input: adk_graph::State,
    #[serde(default = "empty_bindings")]
    model_bindings: Value,
}
async fn preview_run(
    App(b): App<Backend>,
    AuthoringJson(request): AuthoringJson<PreviewRun>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .preview(
                &b.actor(Some(&request.workspace_id)),
                zf_execution::preview::PreviewRun {
                    composition: request.composition,
                    input: request.input,
                    model_bindings: request.model_bindings,
                },
            )
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PrepareRuntimeRequest {
    workspace_id: Option<String>,
    selection: RuntimeSelection,
}
async fn prepare_runtime_value(
    b: &Backend,
    workspace: &Workspace,
    selection: &RuntimeSelection,
) -> anyhow::Result<zf_compiler::prepared_model::PreparedRuntime> {
    b.service
        .prepare(&b.actor(Some(&workspace.id)), selection)
        .await
}
async fn prepare_runtime(
    App(b): App<Backend>,
    Json(request): Json<PrepareRuntimeRequest>,
) -> Api<Value> {
    let workspace = selected_workspace(&b, request.workspace_id.as_deref()).await?;
    let runtime = prepare_runtime_value(&b, &workspace, &request.selection).await?;
    Ok(Json(
        json!({"stage":"prepared","overview":runtime.summary(),"runtime":runtime}),
    ))
}
fn empty_bindings() -> Value {
    json!({})
}
async fn start(App(b): App<Backend>, AuthoringJson(request): AuthoringJson<Start>) -> Api<Value> {
    let actor = b.actor(request.flow.workspace_id.as_deref());
    let definition = if let Some(selection) = request.runtime_selection {
        zf_execution::start::StartDefinition::Composition(selection)
    } else if let Some(key) = request.flow.flow_key {
        zf_execution::start::StartDefinition::Stored {
            key,
            expected_hash: request.flow.flow_hash.context("Flow hash absent")?,
        }
    } else {
        zf_execution::start::StartDefinition::Inline(
            request.flow.composition.context("Choose a flow")?,
        )
    };
    Ok(Json(
        b.service
            .start(
                &actor,
                zf_execution::start::StartRequest {
                    definition,
                    input: request.input,
                    model_bindings: request.model_bindings,
                    node_path: request.node_path,
                    prepared_context: request.prepared_context,
                    preview_metadata: request.preview_metadata,
                },
            )
            .await?,
    ))
}
async fn definition_run(b: &Backend, id: &str) -> anyhow::Result<Value> {
    b.service.definition(&b.run_actor(id).await?, id).await
}
async fn rename_run(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::RenameRun>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .rename(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}
fn node_config<'a>(composition: &'a Value, path: &str) -> Option<&'a Value> {
    let (first, rest) = path
        .split_once('/')
        .map_or((path, None), |(a, b)| (a, Some(b)));
    let node = composition["nodes"]
        .as_array()?
        .iter()
        .find(|n| n["id"] == first)?;
    match rest {
        Some(rest) => node_config(&node["data"]["config"]["composition"], rest),
        None => Some(&node["data"]["config"]),
    }
}
fn node_context_config<'a>(composition: &'a Value, path: &str) -> Option<&'a Value> {
    let config = node_config(composition, path)?;
    if let Some(context_node) = config["contextNode"].as_str() {
        let prefix = path
            .rsplit_once('/')
            .map(|(prefix, _)| format!("{prefix}/"))
            .unwrap_or_default();
        node_config(composition, &format!("{prefix}{context_node}"))
    } else {
        Some(config)
    }
}
#[derive(Deserialize)]
struct Cursor {
    #[serde(default)]
    after: Option<i64>,
}
async fn snapshot(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(cursor): Query<Cursor>,
) -> std::result::Result<impl IntoResponse, ApiError> {
    let value = b.sync.payload(&id, cursor.after.map(|v| v.max(0))).await?;
    Ok(([("Cache-Control", "no-store")], Json(value)))
}
async fn events(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Query(cursor): Query<Cursor>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // EventSource keeps the original URL during automatic reconnects. Its last
    // received event ID therefore takes precedence over an initial ?after= value.
    let after = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .or(cursor.after);
    let changed = b.sync.subscribe();
    let stream = futures::stream::unfold(
        (b, id, after, changed, true),
        |(b, id, after, mut changed, first)| async move {
            if b.sync.shutdown.is_cancelled() {
                return None;
            }
            if !first {
                tokio::select! { _=b.sync.shutdown.cancelled()=>{return None}, _=changed.changed()=>{}, _=tokio::time::sleep(Duration::from_secs(5))=>{} }
            }
            let (event, next, more) = match b.sync.payload(&id, after).await {
                Ok(payload) => {
                    let next = payload["cursor"].as_i64().unwrap_or_default();
                    let head = b
                        .sync
                        .head(&id)
                        .await
                        .map(|(_, revision)| revision)
                        .unwrap_or(next);
                    (
                        Event::default()
                            .event("sync")
                            .data(payload.to_string())
                            .id(next.to_string()),
                        Some(next),
                        next < head,
                    )
                }
                Err(error) => (
                    Event::default()
                        .event("error")
                        .data(json!({"error":error.to_string()}).to_string()),
                    after,
                    false,
                ),
            };
            Some((Ok::<Event, Infallible>(event), (b, id, next, changed, more)))
        },
    );
    (
        [
            ("Cache-Control", "no-store, no-transform"),
            ("X-Accel-Buffering", "no"),
        ],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
}

async fn version_gate(
    App(updates): App<Arc<crate::app_updates::Updates>>,
    request: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let mutation = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    );
    if mutation {
        if let Some(protocol) = request.headers().get("x-zedflow-protocol")
            && protocol.to_str().ok().and_then(|s| s.parse::<u64>().ok())
                != crate::app_updates::build()["protocol"].as_u64()
        {
            return ApiError(
                StatusCode::CONFLICT,
                "Client incompatible avec le protocole du daemon ; actualisez le client".into(),
            )
            .into_response();
        }
        if request.uri().path() == "/api/updates/apply" {
            // A cross-origin browser page cannot initiate a daemon restart.
            if let Some(origin) = request.headers().get("origin") {
                let host = request
                    .headers()
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default();
                if ![format!("http://{host}"), format!("https://{host}")]
                    .iter()
                    .any(|s| Some(s.as_str()) == origin.to_str().ok())
                {
                    return ApiError(
                        StatusCode::FORBIDDEN,
                        "Origine de mise à jour refusée".into(),
                    )
                    .into_response();
                }
            }
            return next.run(request).await;
        }
        let guard = updates.gate.read().await;
        if *guard {
            return ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "Mise à jour en cours ; réessayez après la reconnexion".into(),
            )
            .into_response();
        }
        let response = next.run(request).await;
        drop(guard);
        return response;
    }
    let mut response = next.run(request).await;
    if matches!(response.headers().get("content-type").and_then(|v|v.to_str().ok()),Some(value) if value.contains("json"))
    {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
    }
    response
}
async fn app_version(App(b): App<Backend>) -> Json<Value> {
    let mut status = b.updates.status().await;
    let active:i64=sqlx::query_scalar("SELECT count(*) FROM runs WHERE json_extract(document,'$.status')='running' OR json_extract(document,'$.runtimeActive')=1").fetch_one(&b.db).await.unwrap_or(-1);
    status["activeExecutions"] = json!(if active < 0 { -1 } else { active });
    status["daemonId"] = b.daemon["id"].clone();
    status["instanceId"] = b.daemon["instanceId"].clone();
    Json(status)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppUpdate {
    release_id: String,
    expected_daemon_build_id: String,
}
async fn app_update(App(b): App<Backend>, Json(request): Json<AppUpdate>) -> Api<Value> {
    if !crate::app_updates::valid_id(&request.release_id) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Identifiant de release invalide".into(),
        ));
    }
    let mut gate = b.updates.gate.write().await;
    if *gate {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Une mise à jour est déjà en cours".into(),
        ));
    }
    let maintenance = b
        .service
        .try_begin_maintenance()
        .map_err(|error| ApiError(StatusCode::CONFLICT, error.to_string()))?;
    let ack = b
        .updates
        .request(&request.release_id, &request.expected_daemon_build_id)
        .await
        .map_err(|error| {
            if error.is::<crate::app_updates::RequestConflict>() {
                ApiError(StatusCode::CONFLICT, error.to_string())
            } else if error.is::<crate::app_updates::ReleaseNotFound>() {
                ApiError(StatusCode::NOT_FOUND, "Release absente".into())
            } else {
                ApiError(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Impossible de préparer la mise à jour".into(),
                )
            }
        })?;
    *gate = true;
    let shutdown = b.sync.shutdown.clone();
    tokio::spawn(async move {
        let _maintenance = maintenance;
        tokio::time::sleep(Duration::from_millis(300)).await;
        shutdown.cancel();
    });
    Ok(Json(ack))
}

async fn answer(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::Answer>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .answer(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}

async fn select_model(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::SelectModel>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .select_model(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}

async fn activate_capability(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::ActivateCapability>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .activate_capability(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}

async fn queue_message(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::MessageRequest>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .queue(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}

async fn resume_run(
    App(b): App<Backend>,
    Path(id): Path<String>,
    Json(request): Json<zf_execution::commands::ResumeRequest>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .resume(&b.run_actor(&id).await?, &id, request)
            .await?,
    ))
}
async fn remove_message(
    App(b): App<Backend>,
    Path((id, message)): Path<(String, String)>,
) -> Api<Value> {
    Ok(Json(
        b.service
            .remove_message(&b.run_actor(&id).await?, &id, &message)
            .await?,
    ))
}
async fn abort_run(App(b): App<Backend>, Path(id): Path<String>) -> Api<Value> {
    Ok(Json(b.service.cancel(&b.run_actor(&id).await?, &id).await?))
}

fn node(id: &str, kind: &str, label: &str, x: i32, y: i32, config: Value) -> Value {
    json!({"id":id,"type":"flow","position":{"x":x,"y":y},"data":{"kind":kind,"label":label,"config":config}})
}
fn edge(source: &str, target: &str, branch: Option<&str>) -> Value {
    let mut edge = json!({"id":format!("{source}-{target}"),"source":source,"target":target});
    if let Some(branch) = branch {
        edge["sourceHandle"] = json!(branch);
    }
    edge
}

fn working_system_flow(cwd: &str) -> anyhow::Result<Composition> {
    let tools = ["read", "exec"];
    let context = json!({"modelNode":"model","fanIn":"any","contextStrategy":"working-system-context","contextBindings":zf_context::starters::bindings(),"attachments":{
        "instructions":{"items":[{"id":"workspace","source":{"kind":"workspace"}},{"id":"mission","source":{"kind":"text","text":"Tu interviens dans la documentation du Working System. Réponds à la demande explicite en t’appuyant sur les sources présentes. Le périmètre métier de ce flow reste à préciser ; ne le complète pas par supposition. Cite les fichiers consultés."}}]},
        "skills":{"items":[{"id":"skills","source":{"kind":"workspace"},"activation":"explicit"}]},
        "tools":{"items":tools.iter().map(|name|json!({"id":name,"name":name})).collect::<Vec<_>>()}
    }});
    serde_json::from_value(json!({"formatVersion":3,"id":"working-system","name":"Working System","settings":{"workingDirectory":cwd,"maxConcurrency":1,"recursionLimit":1000},
        "nodes":[
            node("start","start","Début",0,140,json!({"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}}}})),
            node("context","context","Contexte Working System",220,120,context),
            node("model","model","Modèle documentaire",510,120,json!({"contextNode":"context","modelBinding":"runtime","inputField":"input","historyField":"messages","field":"output","toolCallsField":"toolCalls"})),
            node("calls","condition","Outil demandé ?",800,100,json!({"predicate":{"kind":"compare","field":"hasToolCalls","operator":"eq","value":true}})),
            node("tools","tool","Consulter les sources",780,360,json!({"tool":"execute_calls","historyField":"messages","field":"output","toolCallsField":"toolCalls"})),
            node("response","output","Résultat documentaire",1080,130,json!({"text":"{{output}}"})),
            node("end","end","Fin",1340,140,json!({}))
        ],"edges":[edge("start","context",None),edge("context","model",None),edge("model","calls",None),edge("calls","tools",Some("true")),edge("calls","response",Some("false")),edge("tools","context",None),edge("response","end",None)]
    })).map_err(Into::into)
}

fn working_system_harness() -> anyhow::Result<Composition> {
    let mut context_bindings = zf_context::starters::bindings();
    context_bindings["files"] = json!({"kind":"state","field":"workingSystemResult"});
    serde_json::from_value(json!({"formatVersion":3,"id":"working-system-harness","name":"Harness · Working System","settings":{"maxConcurrency":1,"recursionLimit":10000},"channels":[{"name":"workingSystemResult","reducer":"overwrite","default":""}],
        "nodes":[
            node("start","start","Début",0,140,json!({"exports":{"interactive":true,"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}},"branches":{"documentation":{"contract":{"input":{"kind":"text"},"output":{"kind":"text"}},"invocations":["node"]}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"branches":{"documentation":"documentation"}}})),
            node("documentation","route","Consulter Working System",220,120,json!({"branch":"documentation","inputField":"input","field":"workingSystemResult","fanIn":"any"})),
            node("context","context","Préparer la restitution",530,120,json!({"modelNode":"model","contextStrategy":"conversation-default","contextBindings":context_bindings,"attachments":{"instructions":{"items":[{"id":"restitution","source":{"kind":"text","text":"Réponds à l’utilisateur en t’appuyant sur le résultat documentaire fourni par le flow Working System. Préserve ses sources et ses incertitudes."}}]}}})),
            node("model","model","Modèle de restitution",820,120,json!({"contextNode":"context","modelBinding":"runtime","inputField":"input","historyField":"messages","field":"output"})),
            node("response","output","Réponse",1110,130,json!({"text":"{{output}}"})),
            node("inbox","inbox","Suite du travail",1110,380,json!({"field":"input","historyField":"messages","prompt":"Sur quoi continuer ?","responseType":"text"}))
        ],"edges":[edge("start","documentation",None),edge("documentation","context",None),edge("context","model",None),edge("model","response",None),edge("response","inbox",None),edge("inbox","documentation",None)]
    })).map_err(Into::into)
}

async fn ensure_defaults(b: &Backend, workspace: &Workspace) -> anyhow::Result<()> {
    b.service
        .recover_catalog(&b.actor(Some(&workspace.id)))
        .await?;
    let store = zf_storage::context_store::ContextStore::new(workspace.path.clone());
    let files = store.list().await?;
    for (id, name, tools) in [
        (
            "workspace-default",
            "Assistant de workspace",
            &["read", "write", "edit", "exec"][..],
        ),
        ("conversation-default", "Conversation", &[][..]),
        (
            "tools-default",
            "Inspection de données",
            &["inspect_json"][..],
        ),
        (
            "harness-default",
            "Harness de workspace",
            &["read", "write", "edit", "exec"][..],
        ),
    ] {
        if files.iter().any(|file| file.key == id) {
            continue;
        }
        let strategy = if id == "harness-default" {
            zf_context::starters::harness_strategy()
        } else {
            zf_context::starters::strategy(id, name, tools)
        };
        b.service
            .store_source(
                &b.actor(Some(&workspace.id)),
                zf_compiler::programs::SourceOverride {
                    kind: zf_compiler::programs::SourceKind::Strategy,
                    key: id.into(),
                    source: zf_context::context_source::generate(&strategy)
                        .map_err(|d| anyhow::anyhow!("{}", serde_json::json!(d)))?,
                    expected_hash: None,
                },
            )
            .await?;
    }
    Ok(())
}
async fn missing_flow(
    b: &Backend,
    workspace: &Workspace,
    doc: Composition,
) -> anyhow::Result<FlowFile> {
    if let Some(file) = b
        .flows
        .list(workspace)
        .await?
        .into_iter()
        .find(|f| f.id == doc.id && f.scope == "workspace")
    {
        anyhow::ensure!(
            file.composition.is_some(),
            "Invalid existing starter flow: {}",
            file.name
        );
        return Ok(file);
    }
    b.service
        .store_flow(
            &b.actor(Some(&workspace.id)),
            zf_execution::authoring::StoreFlow {
                composition: doc,
                scope: None,
                key: None,
                expected_hash: None,
            },
        )
        .await
}
async fn install_example(b: &Backend, workspace: &Workspace, cwd: &str) -> anyhow::Result<Value> {
    let path = std::path::Path::new(cwd);
    let resolved = tokio::fs::canonicalize(if path.is_absolute() {
        path.to_owned()
    } else {
        workspace.path.join(path)
    })
    .await
    .context("Working System directory not found")?;
    anyhow::ensure!(resolved.is_dir(), "Working System path must be a directory");
    ensure_defaults(b, workspace).await?;
    let contexts = zf_storage::context_store::ContextStore::new(workspace.path.clone());
    if !contexts
        .list()
        .await?
        .iter()
        .any(|f| f.key == "working-system-context")
    {
        let strategy = zf_context::starters::strategy(
            "working-system-context",
            "Working System · documentation",
            &["read", "exec"],
        );
        b.service
            .store_source(
                &b.actor(Some(&workspace.id)),
                zf_compiler::programs::SourceOverride {
                    kind: zf_compiler::programs::SourceKind::Strategy,
                    key: strategy.id.clone(),
                    source: zf_context::context_source::generate(&strategy)
                        .map_err(|d| anyhow::anyhow!("{}", serde_json::json!(d)))?,
                    expected_hash: None,
                },
            )
            .await?;
    }
    for key in ["working-system-context", "conversation-default"] {
        anyhow::ensure!(
            contexts.read(key).await?.strategy.is_some(),
            "Invalid starter context: {key}"
        );
    }
    let worker = missing_flow(b, workspace, working_system_flow(cwd)?).await?;
    let existing = worker
        .composition
        .as_ref()
        .and_then(|d| d.settings.working_directory.as_deref())
        .context("Working directory absent from existing starter")?;
    let existing = std::path::Path::new(existing);
    anyhow::ensure!(
        tokio::fs::canonicalize(if existing.is_absolute() {
            existing.to_owned()
        } else {
            workspace.path.join(existing)
        })
        .await?
            == resolved,
        "Starter already targets another directory"
    );
    let root = missing_flow(b, workspace, working_system_harness()?).await?;
    let bridges = zf_storage::bridge_store::BridgeStore::new(workspace.path.clone())?;
    if !bridges
        .list()
        .await?
        .iter()
        .any(|f| f.key == "working-system")
    {
        use zf_flows::composition::{
            BridgeDefinition, Connection, Endpoint, InvocationKind, RouteMode,
        };
        let bridge = BridgeDefinition::new()
            .import("documentation", &worker.key)
            .connect(
                "documentation",
                Connection::new(
                    Endpoint::new("root", "documentation"),
                    Endpoint::new("documentation", "main"),
                    RouteMode::CallAwait,
                    InvocationKind::Node,
                ),
            );
        b.service
            .store_bridge(
                &b.actor(Some(&workspace.id)),
                zf_execution::authoring::StoreBridge {
                    key: "working-system".into(),
                    bridge,
                    expected_hash: None,
                },
            )
            .await?;
    }
    let bridge = bridges.read("working-system").await?;
    anyhow::ensure!(bridge.bridge.is_some(), "Invalid starter bridge");
    Ok(json!({"root":root,"worker":worker,"bridge":bridge,"workingDirectory":resolved}))
}

/// Explicit local listener and filesystem configuration for the daemon.
pub struct ServerOptions {
    pub listen: std::net::SocketAddr,
    pub data: PathBuf,
    pub workspace: PathBuf,
    pub web: PathBuf,
    pub skill_dirs: Vec<PathBuf>,
    pub flow_home: Option<PathBuf>,
    pub context_home: Option<PathBuf>,
}
/// Serve the web client and API with one execution service until shutdown.
pub async fn serve(args: ServerOptions) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.listen.ip().is_loopback(),
        "Cette première version écoute en local ; utilisez un tunnel SSH pour l'accès distant."
    );
    let shutdown = tokio_util::sync::CancellationToken::new();
    let updates = std::sync::Arc::new(crate::app_updates::Updates::new(
        crate::app_updates::Config::managed(args.web.clone()),
    ));
    let home = args
        .flow_home
        .clone()
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .context("Flow home required")?;
    let home = tokio::fs::canonicalize(home)
        .await
        .context("Resolve flow home")?;
    let service = ExecutionService::open(ExecutionOptions {
        data: args.data.clone(),
        workspace: args.workspace.clone(),
        flow_home: home.clone(),
        context_home: args.context_home.clone(),
        skill_dirs: args.skill_dirs.clone(),
        authorizer: Arc::new(LocalAuthorizer),
    })
    .await?;
    let router = crate::server::router_for_service(
        service.clone(),
        args.data,
        args.workspace,
        args.skill_dirs,
        home,
        args.context_home,
        Some(format!("http://{}", args.listen)),
        shutdown.clone(),
        updates.clone(),
    )
    .await?;
    let client: serde_json::Value = tokio::fs::read(args.web.join("client-version.json"))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let router = if let Some(root) = &updates.config.root {
        router.nest_service(
            "/_client",
            tower_http::services::ServeDir::new(root.join("clients")),
        )
    } else if let Some(id) = client["buildId"]
        .as_str()
        .filter(|id| crate::app_updates::valid_id(id))
    {
        router.nest_service(
            &format!("/_client/{id}"),
            tower_http::services::ServeDir::new(&args.web),
        )
    } else {
        router
    };
    let router = router
        .fallback_service(tower_http::services::ServeDir::new(args.web))
        .layer(axum::middleware::from_fn(
            |request: axum::extract::Request, next: axum::middleware::Next| async move {
                let immutable = request.uri().path().starts_with("/_client/")
                    && request.uri().path().contains("/assets/");
                let mut response = next.run(request).await;
                let cache = if immutable && response.status().is_success() {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-store"
                };
                response.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static(cache),
                );
                response
            },
        ));
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    println!("Zedflow daemon http://{}", listener.local_addr()?);
    let transport_shutdown = service.sync().shutdown.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::select! {_=shutdown_signal()=>{},_=shutdown.cancelled()=>{},_=transport_shutdown.cancelled()=>{}}
            shutdown.cancel();
            transport_shutdown.cancel();
        })
        .await?;
    service.shutdown().await?;
    if updates.requested.load(std::sync::atomic::Ordering::SeqCst) {
        std::process::exit(75);
    }
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

pub(crate) fn error_status(error: &anyhow::Error) -> StatusCode {
    use zf_execution::commands::ExecutionError;
    for cause in error.chain() {
        if let Some(domain) = cause.downcast_ref::<ExecutionError>() {
            return match domain {
                ExecutionError::Busy(_) => StatusCode::SERVICE_UNAVAILABLE,
                ExecutionError::Conflict(_) => StatusCode::CONFLICT,
                ExecutionError::Forbidden(_) => StatusCode::FORBIDDEN,
                ExecutionError::NotFound(_) => StatusCode::NOT_FOUND,
                ExecutionError::Invalid(_) => StatusCode::BAD_REQUEST,
            };
        }
        if cause.is::<zf_storage::flow_store::Conflict>()
            || cause.is::<zf_storage::session_archive::Conflict>()
            || cause.is::<zf_storage::context_store::Conflict>()
            || matches!(
                cause.downcast_ref::<zf_storage::data::DataError>(),
                Some(zf_storage::data::DataError::Conflict { .. })
            )
        {
            return StatusCode::CONFLICT;
        }
        if cause.is::<zf_storage::workspaces::WorkspaceNotFound>() {
            return StatusCode::NOT_FOUND;
        }
    }
    StatusCode::BAD_REQUEST
}
