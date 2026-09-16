//! Workspace-scoped authoring adapters; commands belong to execution.
use axum::{
    Json, Router,
    body::Bytes,
    extract::{FromRequest, Path, Query, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zf_compiler::resolve as composition;
use zf_context::{
    context::{self, ContextLibrary, ContextStrategy},
    context_source,
};
use zf_core::types::{DataType, Diagnostic, TypeRegistry, compatible};
use zf_execution::{commands::Actor, service::ExecutionService};
use zf_flows::composition::{CompositionCatalog, ResolveRequest};
use zf_storage::{
    context_store::{ContextFile, ContextStore, LibraryFile, LibraryStore, TypeFile, TypeStore},
    workspaces,
};

#[derive(Clone)]
struct Authoring {
    db: SqlitePool,
    default_workspace: String,
    flows: zf_storage::flow_store::FlowStore,
    service: ExecutionService,
}

pub(crate) fn router(
    service: ExecutionService,
    flows: zf_storage::flow_store::FlowStore,
) -> Router {
    let db = service.database();
    let default_workspace = service.default_workspace_id().to_owned();
    Router::new()
        .route("/api/context-readers", get(list_readers))
        .route("/api/context-source-types", get(list_source_types))
        .route("/api/graph-analysis", post(graph_analysis))
        .route("/api/composition-analysis", post(composition_analysis))
        .route("/api/type-examples/query", post(query_type_examples))
        .route("/api/type-examples", post(save_type_example))
        .route("/api/type-examples/catalog", get(example_catalog))
        .route("/api/context-packages/export", post(export_package))
        .route("/api/context-packages/validate", post(validate_package))
        .route("/api/context-packages/import", post(import_package))
        .route(
            "/api/context-libraries",
            get(list_libraries).post(save_library),
        )
        .route("/api/context-libraries/{key}", get(read_library))
        .route("/api/context-types", get(list_types).post(save_types))
        .route("/api/context-types/{key}", get(read_types))
        .route("/api/bridges", get(list_bridges).post(save_bridge))
        .route("/api/bridges/{key}", get(read_bridge))
        .route("/api/context-strategies", get(list).post(save))
        .route("/api/context-strategies/validate", post(validate))
        .route("/api/context-strategies/convert", post(convert_strategy))
        .route("/api/context-strategies/preview", post(preview))
        .route("/api/context-strategies/{key}", get(read))
        .route("/api/runtime-graphs/resolve", post(resolve))
        .with_state(Authoring {
            db,
            default_workspace,
            flows,
            service,
        })
}

async fn list_readers() -> Json<Vec<zf_context::resource_readers::ReaderContract>> {
    Json(zf_context::resource_readers::standard_contracts())
}

async fn example_catalog(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<zf_storage::context_store::SourceFile>> {
    Ok(Json(
        zf_storage::source_catalog::examples::catalog(
            authoring_workspace(&state, query.workspace_id.as_deref()).await?,
        )
        .await?,
    ))
}

async fn graph_analysis(
    AuthoringJson(doc): AuthoringJson<zf_flows::schema::Composition>,
) -> Api<zf_flows::node_contracts::GraphAnalysis> {
    if doc.nodes.len() > 250 || doc.edges.len() > 2000 {
        return Err(anyhow::anyhow!("Graph analysis limit exceeded").into());
    }
    Ok(Json(zf_flows::node_contracts::analyze(&doc)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionAnalysis {
    catalog: CompositionCatalog,
    request: ResolveRequest,
}
async fn composition_analysis(
    AuthoringJson(value): AuthoringJson<CompositionAnalysis>,
) -> Json<Value> {
    match composition::resolve(&value.catalog, &value.request) {
        Ok(graph) => Json(json!({"graph":graph,"diagnostics":[]})),
        Err(diagnostics) => Json(json!({"graph":null,"diagnostics":diagnostics})),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypeExampleQuery {
    workspace_id: Option<String>,
    data_type: DataType,
    #[serde(default)]
    types: TypeRegistry,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveTypeExample {
    workspace_id: Option<String>,
    data_type: DataType,
    #[serde(default)]
    types: TypeRegistry,
    label: String,
    value: Value,
}
async fn query_type_examples(
    State(state): State<Authoring>,
    AuthoringJson(query): AuthoringJson<TypeExampleQuery>,
) -> Api<Vec<zf_context::type_examples::TypeExample>> {
    let _read = state
        .service
        .read_scope(&state.actor(query.workspace_id.as_deref()), None)
        .await?;
    let workspace = workspaces::get(
        &state.db,
        query
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    Ok(Json(
        zf_storage::source_catalog::examples::list(workspace.path, query.data_type, query.types)
            .await?,
    ))
}
async fn save_type_example(
    State(state): State<Authoring>,
    AuthoringJson(query): AuthoringJson<SaveTypeExample>,
) -> Api<zf_context::type_examples::TypeExample> {
    Ok(Json(
        state
            .service
            .save_type_example(
                &state.actor(query.workspace_id.as_deref()),
                query.data_type,
                query.types,
                query.label,
                query.value,
            )
            .await?,
    ))
}
async fn list_source_types(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<zf_storage::source_catalog::SourceCatalog> {
    let workspace = workspaces::get(
        &state.db,
        query
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    Ok(Json(
        zf_storage::source_catalog::collect(&state.flows, &workspace).await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConvertStrategyRequest {
    workspace_id: Option<String>,
    strategy: ContextStrategy,
    bindings: Option<BTreeMap<String, zf_context::resources::ResourceBinding>>,
    #[serde(default)]
    formats: BTreeMap<String, context::FragmentFormat>,
}
async fn convert_strategy(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<ConvertStrategyRequest>,
) -> Result<Json<Value>, Error> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    workspaces::get(
        &state.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    Ok(Json(
        match zf_runtime::inference::convert_context_v1(
            &request.strategy,
            request.bindings.as_ref(),
            &request.formats,
        ) {
            Ok(strategy) => json!({"valid":true,"strategy":strategy,"diagnostics":[]}),
            Err(diagnostics) => json!({"valid":false,"diagnostics":diagnostics}),
        },
    ))
}

struct Error(StatusCode, Value);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}
impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        let status = crate::server::error_status(&error);
        Self(status, json!({"error":format!("{error:#}")}))
    }
}
impl From<Vec<Diagnostic>> for Error {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"diagnostics":diagnostics}),
        )
    }
}
type Api<T> = Result<Json<T>, Error>;

/// Authoring accepts the JSON envelope of 64-level schemas. The ordinary Axum
/// extractor's 128-container recursion limit is too small for nested `fields`.
pub(crate) struct AuthoringJson<T>(pub(crate) T);
impl<S, T> FromRequest<S> for AuthoringJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send + 'static,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if content_type != "application/json"
            && !(content_type.starts_with("application/") && content_type.ends_with("+json"))
        {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(json!({"error":"Expected application/json"})),
            )
                .into_response());
        }
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let parsed =
            tokio::task::spawn_blocking(move || zf_context::context_json::from_slice(&bytes))
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error":format!("Context request decoding failed: {error}")})),
                    )
                        .into_response()
                })?;
        parsed.map(Self).map_err(|error| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(
                    json!({"diagnostics":[Diagnostic::new("json_decode", "$", error.to_string())]}),
                ),
            )
                .into_response()
        })
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorkspaceQuery {
    workspace_id: Option<String>,
}

impl Authoring {
    fn actor(&self, workspace: Option<&str>) -> Actor {
        Actor {
            id: "local-http".into(),
            workspace_id: workspace.unwrap_or(&self.default_workspace).into(),
        }
    }
    async fn store(&self, workspace: Option<&str>) -> Result<ContextStore, Error> {
        let id = workspace.unwrap_or(&self.default_workspace);
        let workspace = workspaces::get(&self.db, id).await.map_err(|error| {
            if error
                .downcast_ref::<workspaces::WorkspaceNotFound>()
                .is_some()
            {
                Error(StatusCode::NOT_FOUND, json!({"error":"Workspace inconnu"}))
            } else {
                Error::from(error)
            }
        })?;
        self.service
            .recover_catalog(&self.actor(Some(&workspace.id)))
            .await?;
        Ok(ContextStore::new(workspace.path))
    }
}

async fn list(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<ContextFile>> {
    Ok(Json(
        state
            .store(query.workspace_id.as_deref())
            .await?
            .list()
            .await?,
    ))
}
async fn read(
    State(state): State<Authoring>,
    Path(key): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<ContextFile> {
    Ok(Json(
        state
            .store(query.workspace_id.as_deref())
            .await?
            .read(&key)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveRequest {
    workspace_id: Option<String>,
    strategy: ContextStrategy,
    expected_hash: Option<String>,
    #[serde(default)]
    types: TypeRegistry,
    #[serde(default)]
    library: ContextLibrary,
}
async fn save(
    State(state): State<Authoring>,
    AuthoringJson(mut request): AuthoringJson<SaveRequest>,
) -> Api<ContextFile> {
    let workspace = workspaces::get(
        &state.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    request.strategy.types = context::resolved_types(&request.strategy, &request.types)?;
    context::validate_strategy_with_library(&request.strategy, &request.types, &request.library)?;
    let source = context_source::generate(&request.strategy)?;
    let file = state
        .service
        .store_source(
            &state.actor(Some(&workspace.id)),
            zf_compiler::programs::SourceOverride {
                kind: zf_compiler::programs::SourceKind::Strategy,
                key: request.strategy.id.clone(),
                source,
                expected_hash: request.expected_hash,
            },
        )
        .await?;
    Ok(Json(ContextFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: file.source,
        strategy: Some(request.strategy),
        diagnostics: file.diagnostics,
    }))
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum StrategySelection {
    Draft { strategy: ContextStrategy },
    File { key: String, hash: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectedStrategy {
    strategy: ContextStrategy,
    /// Exact structured Rust used by this preview, including unsaved drafts.
    source: String,
    hash: String,
}

async fn selected(
    store: &ContextStore,
    selection: StrategySelection,
) -> Result<SelectedStrategy, Error> {
    use sha2::{Digest, Sha256};
    match selection {
        StrategySelection::Draft { strategy } => {
            let source = context_source::generate(&strategy)?;
            let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
            Ok(SelectedStrategy {
                strategy,
                source,
                hash,
            })
        }
        StrategySelection::File { key, hash } => {
            let file = store.read(&key).await?;
            if file.hash != hash {
                return Err(Error(
                    StatusCode::CONFLICT,
                    json!({"error":"La stratégie a changé depuis sa sélection"}),
                ));
            }
            let strategy = file.strategy.ok_or_else(|| Error::from(file.diagnostics))?;
            let source = file.source.ok_or_else(|| {
                Error(
                    StatusCode::BAD_REQUEST,
                    json!({"error":"Source de stratégie absente"}),
                )
            })?;
            Ok(SelectedStrategy {
                strategy,
                source,
                hash: file.hash,
            })
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValidateRequest {
    workspace_id: Option<String>,
    selection: StrategySelection,
    #[serde(default)]
    types: TypeRegistry,
    #[serde(default)]
    library: ContextLibrary,
    #[serde(default)]
    resources: BTreeMap<String, DataType>,
    #[serde(default)]
    granted_capabilities: BTreeSet<String>,
}

fn validate_binding(
    strategy: &ContextStrategy,
    types: &TypeRegistry,
    library: &ContextLibrary,
    resources: &BTreeMap<String, DataType>,
    granted: &BTreeSet<String>,
) -> Result<(), Vec<Diagnostic>> {
    let types = context::resolved_types(strategy, types)?;
    let mut errors = context::validate_strategy_with_library(strategy, &types, library)
        .err()
        .unwrap_or_default();
    for (name, required) in &strategy.requirements {
        match resources.get(name) {
            Some(available) if compatible(available, required, &types) => {}
            Some(_) => errors.push(Diagnostic::new(
                "resource_type",
                format!("requirements.{name}"),
                "La donnée exposée est incompatible avec la stratégie",
            )),
            None => errors.push(Diagnostic::new(
                "resource_binding",
                format!("requirements.{name}"),
                "La ressource doit être déclarée dans le scope du nœud",
            )),
        }
    }
    errors.extend(capability_errors(strategy, granted));
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn capability_errors(strategy: &ContextStrategy, granted: &BTreeSet<String>) -> Vec<Diagnostic> {
    strategy
        .capabilities
        .iter()
        .filter(|capability| !granted.contains(&capability.id))
        .map(|capability| {
            Diagnostic::new(
                "capability_not_granted",
                format!("capabilities.{}", capability.id),
                "La stratégie demande une capacité que le flow n’accorde pas",
            )
        })
        .collect()
}

async fn validate(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<ValidateRequest>,
) -> Api<Value> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    let store = state.store(request.workspace_id.as_deref()).await?;
    let selected = selected(&store, request.selection).await?;
    validate_binding(
        &selected.strategy,
        &request.types,
        &request.library,
        &request.resources,
        &request.granted_capabilities,
    )?;
    Ok(Json(json!({"valid":true,"selection":selected})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewRequest {
    workspace_id: Option<String>,
    selection: StrategySelection,
    #[serde(default)]
    types: TypeRegistry,
    #[serde(default)]
    library: ContextLibrary,
    #[serde(default)]
    resources: BTreeMap<String, Value>,
    #[serde(default)]
    #[serde(rename = "grantedCapabilities")]
    _granted_capabilities: BTreeSet<String>,
    #[serde(default)]
    profile: Option<zf_runtime::resources::request_preview::PreviewProfile>,
}
async fn preview(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<PreviewRequest>,
) -> Api<Value> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    let store = state.store(request.workspace_id.as_deref()).await?;
    let selected = selected(&store, request.selection).await?;
    context::validate_strategy_with_library(&selected.strategy, &request.types, &request.library)?;
    // An authoring preview has no enclosing flow and cannot execute capabilities.
    // Requested selections are returned for inspection; validate_binding and the
    // runtime still require explicit grants before a strategy can be used.
    // Ownership moves into shared immutable inputs. A preview neither persists
    // fixtures nor reads the personal run state or workspace files implicitly.
    let resources = request
        .resources
        .into_iter()
        .map(|(name, value)| (name, Arc::new(value)))
        .collect();
    let evaluation = context::evaluate_with_library(
        &selected.strategy,
        &resources,
        &request.types,
        &request.library,
    );
    let prepared_request = if let Some(profile) = &request.profile {
        let program = zf_context::request_preview::program(
            selected.strategy.clone(),
            selected.source.clone(),
            selected.hash.clone(),
            request.types,
            request.library,
        );
        Some(
            zf_runtime::resources::request_preview::prepare(
                profile,
                &program,
                &evaluation,
                &resources,
            )
            .await,
        )
    } else {
        None
    };
    Ok(Json(
        json!({"selection":selected,"evaluation":evaluation,"request":prepared_request}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolveComposition {
    workspace_id: Option<String>,
    catalog: CompositionCatalog,
    request: ResolveRequest,
    #[serde(default)]
    contexts: BTreeMap<String, StrategySelection>,
    #[serde(default)]
    library: ContextLibrary,
}
async fn resolve(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<ResolveComposition>,
) -> Api<Value> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    let store = state.store(request.workspace_id.as_deref()).await?;
    let graph = composition::resolve(&request.catalog, &request.request)?;
    let mut selections = request.contexts;
    for (path, inference) in &graph.inferences {
        if !selections.contains_key(path)
            && let Some(key) = &inference.definition.context_strategy
        {
            let file = store.read(key).await?;
            selections.insert(
                path.clone(),
                StrategySelection::File {
                    key: key.clone(),
                    hash: file.hash,
                },
            );
        }
    }
    let mut contexts = BTreeMap::new();
    let mut errors = Vec::new();
    for (path, selection) in selections {
        let Some(inference) = graph.inferences.get(&path) else {
            errors.push(Diagnostic::new(
                "inference_missing",
                format!("contexts.{path}"),
                "Nœud d’inférence absent du graphe résolu",
            ));
            continue;
        };
        let selected = selected(&store, selection).await?;
        let flow = &graph.instances[&inference.instance].definition;
        let resources = inference
            .definition
            .resources
            .iter()
            .filter_map(|name| {
                flow.data
                    .get(name)
                    .filter(|data| data.permissions.read)
                    .map(|data| (name.clone(), data.data_type.clone()))
                    .or_else(|| {
                        flow.requires
                            .get(name)
                            .filter(|data| data.permissions.read)
                            .map(|data| (name.clone(), data.data_type.clone()))
                    })
            })
            .collect();
        if let Err(diagnostics) = validate_binding(
            &selected.strategy,
            &graph.types,
            &request.library,
            &resources,
            &inference.definition.capabilities,
        ) {
            errors.extend(diagnostics.into_iter().map(|mut d| {
                d.path = format!("contexts.{path}.{}", d.path);
                d
            }));
        }
        contexts.insert(path, selected);
    }
    if !errors.is_empty() {
        return Err(errors.into());
    }
    Ok(Json(
        json!({"stage":"resolved","graph":graph,"contexts":contexts}),
    ))
}

async fn authoring_workspace(
    state: &Authoring,
    id: Option<&str>,
) -> Result<std::path::PathBuf, Error> {
    let workspace = workspaces::get(&state.db, id.unwrap_or(&state.default_workspace))
        .await
        .map_err(|error| {
            if error
                .downcast_ref::<workspaces::WorkspaceNotFound>()
                .is_some()
            {
                Error(StatusCode::NOT_FOUND, json!({"error":"Workspace inconnu"}))
            } else {
                Error::from(error)
            }
        })?;
    state
        .service
        .recover_catalog(&state.actor(Some(&workspace.id)))
        .await?;
    Ok(workspace.path)
}
async fn list_libraries(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<LibraryFile>> {
    Ok(Json(
        LibraryStore::new(authoring_workspace(&state, query.workspace_id.as_deref()).await?)
            .list()
            .await?,
    ))
}
async fn read_library(
    State(state): State<Authoring>,
    Path(key): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<LibraryFile> {
    Ok(Json(
        LibraryStore::new(authoring_workspace(&state, query.workspace_id.as_deref()).await?)
            .read(&key)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveLibrary {
    workspace_id: Option<String>,
    key: String,
    library: ContextLibrary,
    expected_hash: Option<String>,
    #[serde(default)]
    types: TypeRegistry,
}
async fn save_library(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<SaveLibrary>,
) -> Api<LibraryFile> {
    context::validate_library(&request.library, &request.types)?;
    let workspace = workspaces::get(
        &state.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    let source = context_source::generate_library(&request.library)?;
    let file = state
        .service
        .store_source(
            &state.actor(Some(&workspace.id)),
            zf_compiler::programs::SourceOverride {
                kind: zf_compiler::programs::SourceKind::Library,
                key: request.key,
                source,
                expected_hash: request.expected_hash,
            },
        )
        .await?;
    Ok(Json(LibraryFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: file.source,
        library: Some(request.library),
        diagnostics: file.diagnostics,
    }))
}
async fn list_types(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<TypeFile>> {
    Ok(Json(
        TypeStore::new(authoring_workspace(&state, query.workspace_id.as_deref()).await?)
            .list()
            .await?,
    ))
}
async fn read_types(
    State(state): State<Authoring>,
    Path(key): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<TypeFile> {
    Ok(Json(
        TypeStore::new(authoring_workspace(&state, query.workspace_id.as_deref()).await?)
            .read(&key)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveTypes {
    workspace_id: Option<String>,
    key: String,
    types: TypeRegistry,
    expected_hash: Option<String>,
}
async fn save_types(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<SaveTypes>,
) -> Api<TypeFile> {
    let workspace = workspaces::get(
        &state.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    let source = context_source::generate_types(&request.types)?;
    let file = state
        .service
        .store_source(
            &state.actor(Some(&workspace.id)),
            zf_compiler::programs::SourceOverride {
                kind: zf_compiler::programs::SourceKind::Types,
                key: request.key,
                source,
                expected_hash: request.expected_hash,
            },
        )
        .await?;
    Ok(Json(TypeFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: file.source,
        types: Some(request.types),
        diagnostics: file.diagnostics,
    }))
}

async fn list_bridges(
    State(state): State<Authoring>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<Vec<zf_storage::bridge_store::BridgeFile>> {
    Ok(Json(
        zf_storage::bridge_store::BridgeStore::new(
            authoring_workspace(&state, query.workspace_id.as_deref()).await?,
        )?
        .list()
        .await?,
    ))
}
async fn read_bridge(
    State(state): State<Authoring>,
    Path(key): Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Api<zf_storage::bridge_store::BridgeFile> {
    Ok(Json(
        zf_storage::bridge_store::BridgeStore::new(
            authoring_workspace(&state, query.workspace_id.as_deref()).await?,
        )?
        .read(&key)
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveBridge {
    workspace_id: Option<String>,
    key: String,
    bridge: zf_flows::composition::BridgeDefinition,
    expected_hash: Option<String>,
}
async fn save_bridge(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<SaveBridge>,
) -> Api<zf_storage::bridge_store::BridgeFile> {
    let workspace = workspaces::get(
        &state.db,
        request
            .workspace_id
            .as_deref()
            .unwrap_or(&state.default_workspace),
    )
    .await?;
    let file = state
        .service
        .store_bridge(
            &state.actor(Some(&workspace.id)),
            zf_execution::authoring::StoreBridge {
                key: request.key,
                bridge: request.bridge.clone(),
                expected_hash: request.expected_hash,
            },
        )
        .await?;
    Ok(Json(zf_storage::bridge_store::BridgeFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: file.source,
        bridge: Some(request.bridge),
        diagnostics: file.diagnostics,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageSelectionRequest {
    workspace_id: Option<String>,
    artifacts: Vec<zf_context::context_package::ArtifactSelection>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageRequest {
    workspace_id: Option<String>,
    package: zf_context::context_package::ContextPackage,
}
async fn export_package(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<PackageSelectionRequest>,
) -> Api<zf_context::context_package::ContextPackage> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    Ok(Json(
        zf_storage::context_store::packages::export_selection(
            authoring_workspace(&state, request.workspace_id.as_deref()).await?,
            &request.artifacts,
        )
        .await?,
    ))
}
async fn validate_package(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<PackageRequest>,
) -> Api<zf_context::context_package::PackageValidation> {
    let _read = state
        .service
        .read_scope(&state.actor(request.workspace_id.as_deref()), None)
        .await?;
    authoring_workspace(&state, request.workspace_id.as_deref()).await?;
    Ok(Json(zf_context::context_package::validate_package(
        &request.package,
        &zf_flows::bridge_source::PackageBridgeValidator,
    )))
}
async fn import_package(
    State(state): State<Authoring>,
    AuthoringJson(request): AuthoringJson<PackageRequest>,
) -> Api<zf_storage::context_store::packages::PackageImport> {
    Ok(Json(
        state
            .service
            .import_context_package(
                &state.actor(request.workspace_id.as_deref()),
                &request.package,
            )
            .await?,
    ))
}
