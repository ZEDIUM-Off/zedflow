//! Local ownership of executions and their durable command boundary.
use crate::{
    commands::{self, Actor, CommandAuthorizer, CommandKind},
    revisions::revision_definitions,
    sessions::{apply_activity, apply_observation, persist_batch, persist_command, persist_event},
};
use adk_graph::prelude::*;
use anyhow::{Context, Result, ensure};
use commands::{command_load, run_node_kind};
use futures::StreamExt;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock};
use uuid::Uuid;
use zf_flows::schema::Composition;
use zf_runtime::{
    materialize as compiler, runtime::RunServices, workspace_context::ContextSnapshot,
};
use zf_storage::{
    content_store::ContentStore, flow_store::FlowStore, session_sync::SessionSync, workspaces,
};

/// Explicit filesystem and caller policy configuration; construction does not
/// start a flow or resume any previously interrupted execution.
pub struct ExecutionOptions {
    pub data: PathBuf,
    pub workspace: PathBuf,
    pub flow_home: PathBuf,
    pub context_home: Option<PathBuf>,
    pub skill_dirs: Vec<PathBuf>,
    pub authorizer: Arc<dyn CommandAuthorizer>,
}
#[derive(Clone)]
pub struct ExecutionService {
    pub(crate) state: Arc<ExecutionState>,
}
pub(crate) struct ExecutionState {
    pub(crate) db: sqlx::SqlitePool,
    pub(crate) writer_db: sqlx::SqlitePool,
    pub(crate) content: ContentStore,
    pub(crate) sync: SessionSync,
    pub(crate) sequence: Arc<AtomicI64>,
    pub(crate) data: PathBuf,
    pub(crate) writer: Arc<Mutex<()>>,
    pub(crate) authoring_writer: Arc<Mutex<()>>,
    pub(crate) services: Arc<Mutex<HashMap<String, Arc<RunServices>>>>,
    executions: Arc<Mutex<HashMap<String, ExecutionControl>>>,
    pub(crate) context: ContextSnapshot,
    pub(crate) skill_dirs: Vec<PathBuf>,
    pub(crate) default_workspace_id: String,
    pub(crate) flows: FlowStore,
    pub(crate) home: PathBuf,
    pub(crate) context_home: Option<PathBuf>,
    maintenance: Arc<RwLock<()>>,
    closed: AtomicBool,
    owner_lock: zf_storage::migration::DataLock,
    migration_lock: zf_storage::migration::DataLock,
    launches: std::sync::Mutex<HashMap<String, usize>>,
    launch_changes: tokio::sync::watch::Sender<()>,
    authorizer: Arc<dyn CommandAuthorizer>,
}
/// The lease follows the owner task, including launched children, until its
/// observations are committed. Only command entrypoints can construct it.
#[derive(Clone)]
pub(crate) struct ExecutionContext {
    state: Arc<ExecutionState>,
    _lease: Arc<OwnedRwLockReadGuard<()>>,
    pub(crate) actor: Actor,
}
impl std::ops::Deref for ExecutionContext {
    type Target = ExecutionState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
/// Holding this guard excludes all admitted commands and active owner tasks.
pub struct MaintenanceGuard {
    _guard: OwnedRwLockWriteGuard<()>,
}
#[derive(Clone)]
struct ExecutionControl {
    resumes: tokio::sync::mpsc::UnboundedSender<ExecutionResume>,
    finished: tokio::sync::watch::Receiver<bool>,
}
struct ExecutionResume {
    input: State,
    checkpoint: Option<String>,
    claim_id: Option<String>,
    accepted: tokio::sync::oneshot::Sender<()>,
}
impl ExecutionService {
    pub async fn open(options: ExecutionOptions) -> Result<Self> {
        let requested_data = if options.data.is_absolute() {
            options.data.clone()
        } else {
            std::env::current_dir()?.join(&options.data)
        };
        tokio::fs::create_dir_all(
            requested_data
                .parent()
                .context("Data directory has no parent")?,
        )
        .await?;
        // This sibling lock survives the directory swaps used by maintenance.
        // Acquire it before recovery or opening any database inside the directory.
        let migration_lock = zf_storage::migration::lock(&requested_data).map_err(|error| {
            commands::ExecutionError::Busy(format!(
                "Execution storage already owned or unavailable: {error}"
            ))
        })?;
        zf_storage::migration::recover(&requested_data).await?;
        tokio::fs::create_dir_all(&requested_data).await?;
        let metadata = tokio::fs::symlink_metadata(&requested_data).await?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Data directory must be an ordinary directory"
        );
        let data = tokio::fs::canonicalize(&requested_data).await?;
        // Exclusive OS ownership prevents a second service instance from
        // recovering or admitting runs against this same active storage.
        let owner_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(data.join("execution.lock"))?;
        let owner_lock =
            zf_storage::migration::DataLock::try_lock(owner_lock).map_err(|error| {
                commands::ExecutionError::Busy(format!(
                    "Execution storage already owned or unavailable: {error}"
                ))
            })?;
        let workspace = tokio::fs::canonicalize(options.workspace).await?;
        let context = ContextSnapshot::load_with_home(
            &workspace,
            &options.skill_dirs,
            options.context_home.as_deref(),
        )
        .await?;
        let sqlite = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(data.join("zedflow.db"))
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));
        let writer_db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .connect_with(sqlite.clone())
            .await?;
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(sqlite)
            .await?;
        zf_storage::legacy_compositions::require_imported(&db, &data).await?;
        let content = ContentStore::new(writer_db.clone()).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS runs(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
            .execute(&writer_db)
            .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT, run TEXT NOT NULL, document TEXT NOT NULL)").execute(&writer_db).await?;
        zf_storage::session_store::initialize(&writer_db).await?;
        zf_storage::session_archive::recover_imports(&db, &data).await?;
        workspaces::initialize(&db).await?;
        let initial = workspaces::open(&db, &workspace).await?;
        let sequence = sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(seq),0) FROM events")
            .fetch_one(&db)
            .await?;
        let sync = SessionSync::new(
            db.clone(),
            content.clone(),
            Arc::new(zf_runtime::archive_validation::RuntimeArchiveValidation),
        );
        let service = Self {
            state: Arc::new(ExecutionState {
                db,
                writer_db,
                content,
                sync,
                sequence: Arc::new(AtomicI64::new(sequence)),
                data,
                writer: Arc::new(Mutex::new(())),
                authoring_writer: Arc::new(Mutex::new(())),
                services: Arc::new(Mutex::new(HashMap::new())),
                executions: Arc::new(Mutex::new(HashMap::new())),
                context,
                skill_dirs: options.skill_dirs,
                default_workspace_id: initial.id,
                flows: FlowStore::new(
                    options.flow_home.clone(),
                    Arc::new(zf_compiler::graph_compiler::GraphValidator::new(
                        &compiler::RuntimePrimitives,
                    )),
                ),
                home: options.flow_home,
                context_home: options.context_home,
                maintenance: Arc::new(RwLock::new(())),
                authorizer: options.authorizer,
                owner_lock,
                migration_lock,
                closed: AtomicBool::new(false),
                launches: std::sync::Mutex::new(HashMap::new()),
                launch_changes: tokio::sync::watch::channel(()).0,
            }),
        };
        service.recover_interrupted().await?;
        Ok(service)
    }
    async fn recover_interrupted(&self) -> Result<()> {
        let ids:Vec<String>=sqlx::query_scalar("SELECT id FROM runs WHERE json_extract(document,'$.status')='running' OR json_extract(document,'$.runtimeActive')=1").fetch_all(&self.state.db).await?;
        for id in ids {
            let mut run = zf_storage::session_store::load_projection(&self.state.db, &id).await?;
            // ADK commits before its observation is published. Recover only
            // durable checkpoints belonging to this run and its child threads.
            let checkpoints = zf_runtime::stored_checkpointer::StoredCheckpointer::new(
                zf_storage::contracts::CheckpointStore::new(self.state.content.clone()).await?,
            );
            let headers = checkpoints.storage().list_run_headers(&id).await?;
            let mut latest = std::collections::BTreeMap::new();
            // Storage orders by created_at, then its monotone commit sequence.
            for header in &headers {
                latest.insert(header.thread_id.clone(), header);
            }
            for header in latest.values() {
                // Validate immutable content/header integrity before publishing
                // its references; a damaged checkpoint must not become a frontier.
                checkpoints.storage().hydrate(header).await?;
                let mut event = serde_json::to_value(header)?;
                event["type"] = json!("checkpoint_committed");
                if header.thread_id == id {
                    run.as_object_mut()
                        .context("Run projection is not an object")?
                        .remove("state");
                }
                apply_activity(&mut run, &event);
            }
            if let Some(resume) = run["resumeCheckpoint"].as_str()
                && let Some(frontier) = latest.get(&id)
                && resume != frontier.checkpoint_id
            {
                anyhow::ensure!(
                    headers
                        .iter()
                        .any(|header| header.thread_id == id && header.checkpoint_id == resume),
                    "Resume checkpoint absent from the recovered run"
                );
                run["resumeInput"] = Value::Null;
                run["resumeCheckpoint"] = Value::Null;
            }
            let consumed = run["consumedMessages"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for message in run["queue"].as_array_mut().into_iter().flatten() {
                if message["status"] == "pending" && consumed.contains(&message["id"]) {
                    message["status"] = json!("consumed");
                }
            }
            if run["status"] == "running" {
                run["status"] = json!("interrupted");
            }
            run["runtimeActive"] = json!(false);
            run["activeNodes"] = json!([]);
            run["activeNode"] = Value::Null;
            for activity in run["activities"].as_array_mut().into_iter().flatten() {
                if activity["status"] == "running" {
                    activity["status"] = json!("interrupted");
                }
            }
            self.state.sync.seed(&id).await?;
            persist_command(
                &self.state,
                &id,
                &run,
                &json!({"type":"run_status","status":run["status"],"reason":"process_restarted"}),
            )
            .await?;
        }
        Ok(())
    }
    pub fn default_workspace_id(&self) -> &str {
        &self.state.default_workspace_id
    }
    pub fn sync(&self) -> SessionSync {
        self.state.sync.clone()
    }
    pub fn content(&self) -> ContentStore {
        self.state.content.clone()
    }
    pub fn database(&self) -> sqlx::SqlitePool {
        self.state.db.clone()
    }
    /// Fails while commands or executions are active. It never waits ahead of a
    /// human answer or cancellation that an active task may still need.
    pub fn try_begin_maintenance(&self) -> Result<MaintenanceGuard> {
        self.state
            .maintenance
            .clone()
            .try_write_owned()
            .map(|guard| MaintenanceGuard { _guard: guard })
            .map_err(|_| {
                commands::ExecutionError::Busy("Des exécutions ou commandes sont actives".into())
                    .into()
            })
    }
    /// Admission for operations which must exclude every execution and command.
    /// Never queue a writer ahead of a human answer needed by an active run.
    pub(crate) async fn admit_maintenance(
        &self,
        actor: &Actor,
        kind: CommandKind,
    ) -> Result<(MaintenanceGuard, workspaces::Workspace)> {
        let guard = self.try_begin_maintenance()?;
        ensure!(
            !self.state.closed.load(Ordering::Acquire),
            commands::ExecutionError::Busy("Service arrêté".into())
        );
        let workspace = workspaces::get(&self.state.db, &actor.workspace_id).await?;
        self.state
            .authorizer
            .authorize(actor, kind, &workspace, None)
            .await?;
        Ok((guard, workspace))
    }
    pub(crate) fn has_active_run(&self, id: &str) -> bool {
        self.state
            .launches
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains_key(id)
    }
    /// Resource-scope preflight for artifact capture, after actor admission.
    /// A foreign and an absent run have the same externally visible identity.
    pub(crate) async fn require_run_scope(&self, actor: &Actor, id: &str) -> Result<()> {
        match self.state.sync.head(id).await {
            Ok((workspace, _)) if workspace.as_str() == Some(actor.workspace_id.as_str()) => Ok(()),
            Ok(_) => Err(commands::ExecutionError::NotFound(
                "Session absente de ce workspace".into(),
            )
            .into()),
            Err(error)
                if error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<sqlx::Error>(),
                        Some(sqlx::Error::RowNotFound)
                    )
                }) =>
            {
                Err(
                    commands::ExecutionError::NotFound("Session absente de ce workspace".into())
                        .into(),
                )
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) async fn admit(
        &self,
        actor: &Actor,
        kind: CommandKind,
        run_id: Option<&str>,
    ) -> Result<ExecutionContext> {
        let lease = self
            .state
            .maintenance
            .clone()
            .try_read_owned()
            .map_err(|_| commands::ExecutionError::Busy("Maintenance en cours".into()))?;
        ensure!(
            !self.state.closed.load(Ordering::Acquire),
            commands::ExecutionError::Busy("Service arrêté".into())
        );
        let workspace = workspaces::get(&self.state.db, &actor.workspace_id).await?;
        let run = if let Some(id) = run_id {
            let (run, _) = self.state.sync.latest(id).await?;
            ensure!(
                run["workspaceId"] == actor.workspace_id,
                commands::ExecutionError::Forbidden(
                    "Cette exécution appartient à un autre workspace".into()
                )
            );
            Some(run)
        } else {
            None
        };
        self.state
            .authorizer
            .authorize(actor, kind, &workspace, run.as_ref())
            .await?;
        Ok(ExecutionContext {
            state: self.state.clone(),
            _lease: Arc::new(lease),
            actor: actor.clone(),
        })
    }
    pub async fn read(&self, actor: &Actor, id: &str) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Read, Some(id)).await?;
        zf_storage::session_store::load(&b.db, id).await
    }
    /// Frozen definition detail without hydrating the conversation or tool output.
    pub async fn definition(&self, actor: &Actor, id: &str) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Read, Some(id)).await?;
        commands::definition_run(&b, id).await
    }
    pub async fn wait_idle(&self, id: &str) -> Result<()> {
        let mut changes = self.state.launch_changes.subscribe();
        loop {
            let pending = self
                .state
                .launches
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .contains_key(id);
            if !pending {
                return Ok(());
            }
            changes
                .changed()
                .await
                .context("Execution ownership notifications closed")?;
        }
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.state.closed.store(true, Ordering::Release);
        for service in self.state.services.lock().await.values() {
            service.cancel.cancel();
        }
        // Previously admitted commands retain their lease until their owners
        // drain. services_for also cancels any service created after closure.
        let _exclusive = self.state.maintenance.write().await;
        self.state.sync.shutdown.cancel();
        self.state.writer_db.close().await;
        self.state.db.close().await;
        self.state.owner_lock.unlock()?;
        self.state.migration_lock.unlock()?;
        Ok(())
    }
}
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

struct LaunchLease {
    state: Arc<ExecutionState>,
    id: String,
}
impl Drop for LaunchLease {
    fn drop(&mut self) {
        let mut launches = self
            .state
            .launches
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(count) = launches.get_mut(&self.id) {
            *count -= 1;
            if *count == 0 {
                launches.remove(&self.id);
            }
        }
        self.state.launch_changes.send_replace(());
    }
}
pub(crate) fn launch(
    b: ExecutionContext,
    id: String,
    input: adk_graph::State,
    resume: Option<String>,
    claim_id: Option<String>,
) {
    // Register before spawning: wait_idle observes even a task not yet polled.
    *b.launches
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .entry(id.clone())
        .or_default() += 1;
    let lease = LaunchLease {
        state: b.state.clone(),
        id: id.clone(),
    };
    tokio::spawn(async move {
        let _lease = lease;
        let (receiver, finished) = loop {
            let mut owners = b.executions.lock().await;
            if let Some(owner) = owners.get(&id).cloned() {
                drop(owners);
                let (accepted, acknowledgement) = tokio::sync::oneshot::channel();
                let request = ExecutionResume {
                    input: input.clone(),
                    checkpoint: resume.clone(),
                    claim_id: claim_id.clone(),
                    accepted,
                };
                if owner.resumes.send(request).is_ok() && acknowledgement.await.is_ok() {
                    return;
                }
                // An answer can race the last child settling. Only an accepted
                // request belongs to the old owner; otherwise wait until all of
                // its observations are committed before creating its successor.
                let mut finished = owner.finished;
                let _ = finished.wait_for(|done| *done).await;
                continue;
            }
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            let (finished, completion) = tokio::sync::watch::channel(false);
            owners.insert(
                id.clone(),
                ExecutionControl {
                    resumes: sender,
                    finished: completion,
                },
            );
            break (receiver, finished);
        };
        if let Err(error) = execute(&b, &id, input, resume, receiver, claim_id).await
            && let Ok((mut run, _)) = b.sync.latest(&id).await
        {
            run["status"] = json!(if b.closed.load(Ordering::Acquire) {
                "interrupted"
            } else {
                "error"
            });
            run["runtimeActive"] = json!(false);
            run["error"] = json!(format!("{error:#}"));
            run["activeNodes"] = json!([]);
            run["activeNode"] = Value::Null;
            if let Some(activities) = run["activities"].as_array_mut() {
                for a in activities.iter_mut().filter(|a| a["status"] == "running") {
                    a["status"] = json!("interrupted");
                }
            }
            if let Err(e) = persist_event(
                &b,
                &id,
                &run,
                &json!({"type":"run_status","status":run["status"],"error":run["error"]}),
            )
            .await
            {
                eprintln!("Cannot persist run failure: {e}");
            }
        }
        b.executions.lock().await.remove(&id);
        let _ = finished.send(true);
    });
}
async fn execute(
    b: &ExecutionContext,
    id: &str,
    input: adk_graph::State,
    resume: Option<String>,
    mut resumes: tokio::sync::mpsc::UnboundedReceiver<ExecutionResume>,
    claim_id: Option<String>,
) -> anyhow::Result<()> {
    let (mut run, services) = {
        let _writer = b.writer.lock().await;
        let run = command_load(b, id).await?;
        let services = services_for(b, id, &run).await?;
        (run, services)
    };
    let previous_activities = run["activities"].as_array().map_or(0, Vec::len);
    let mut doc: Composition = serde_json::from_value(run["composition"].clone())?;
    if run["abortRequested"] == true {
        services.cancel.cancel();
    }
    let (sender, mut observed) = zf_runtime::event_sink::channel(64);
    services.set_content_store(b.content.clone());
    services.set_data_registry(
        zf_storage::data::DataRegistry::new(b.content.pool().clone(), b.content.clone(), id)
            .await?,
    )?;
    services.set_sender(Some(sender.clone()));
    let cp = zf_runtime::stored_checkpointer::StoredCheckpointer::new(
        zf_storage::contracts::CheckpointStore::new(b.content.clone()).await?,
    )
    .with_sender(sender.clone());
    let route_runtime = if let Some(prepared) = run.get("runtimeGraph").filter(|v| !v.is_null()) {
        let runtime = crate::route_runtime::RouteRuntime::new(
            serde_json::from_value(prepared.clone())?,
            &services,
            Arc::new(cp.clone()),
            Some(sender.clone()),
        )?;
        services.set_dynamic_capabilities(runtime.clone());
        Some(runtime)
    } else {
        None
    };
    let cleanup_runtime = route_runtime.clone();
    let cleanup_services = services.clone();
    let current_claim = Arc::new(std::sync::Mutex::new(claim_id));
    let mut producer: Option<tokio_util::task::AbortOnDropHandle<()>> = None;
    let result:Result<()>=async {
    let scope = if route_runtime.is_some() { "root/" } else { "" };
    let mut definitions = revision_definitions(&run)?;
    let runtime_entry = run["runtimeGraph"]["graph"]["entry"]["port"]
        .as_str()
        .map(str::to_owned);
    if let Some(checkpoint_id) = &resume {
        let checkpoint = cp
            .load_by_id(checkpoint_id)
            .await?
            .context("Resume checkpoint absent")?;
        anyhow::ensure!(
            checkpoint.thread_id == id,
            "Resume checkpoint belongs to another run"
        );
        if let Some(definition) = zf_runtime::revisions::checkpoint_definition(
            &b.content,
            id,
            id,
            checkpoint.step,
            scope.trim_end_matches('/'),
        )
        .await?
        {
            doc = if let Some(entry) = &runtime_entry {
                zf_flows::flow_contract::at_entry(&definition.composition, entry)?
            } else {
                definition.composition.clone()
            };
            definitions.insert(scope.trim_end_matches('/').into(), definition);
            services.set_revisions(
                zf_runtime::revisions::RevisionRuntime::new(
                    b.content.clone(),
                    id,
                    definitions.clone(),
                )
                .await?,
            )?;
        }
    }
    let graph = compiler::build_scope(
        &doc,
        Some(sender.clone()),
        scope,
        Some(services.clone()),
        Some(Arc::new(cp.clone())),
    )?;
    let mut config = ExecutionConfig::new(id);
    if let Some(limit) = run["composition"]["settings"]["recursionLimit"].as_u64() {
        config = config.with_recursion_limit(limit as usize);
    }
    if let Some(checkpoint) = resume {
        config = config.with_resume_from(&checkpoint);
    }
    // Graph polling is independent of projection writes and of browser consumers.
    run = zf_storage::session_store::compact_run(&b.content, &run).await?;
    let (graph_events, mut stream) = tokio::sync::mpsc::channel(64);
    let producer_services = services.clone();
    let producer_cp = cp.clone();
    let producer_sender = sender.clone();
    let producer_id = id.to_owned();
    let live_routes = route_runtime.clone();
    let producer_claim=current_claim.clone();
    producer = Some(tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        let result:anyhow::Result<()>=async {
            // Initialize only after compilation and with the observation
            // consumer running. Recovered launches then have the same owner.
            if let Some(runtime) = &route_runtime { runtime.initialize(&input).await?; }
            let mut graph=graph;
            let mut input=input;
            let mut config=config;
            let mut launches: Option<tokio_util::task::AbortOnDropHandle<anyhow::Result<()>>> = None;
            loop {
                let (terminal,interrupted)={
                    let events=graph.stream(input,config.clone(),StreamMode::Debug);
                    futures::pin_mut!(events);
                    let mut terminal=None;
                    let mut interrupted=None;
                    while let Some(event)=events.next().await {
                        if matches!(&event,Ok(StreamEvent::Done{..})){terminal=Some(event);break;}
                        if matches!(&event,Ok(StreamEvent::Interrupted{..})){interrupted=Some(event);continue;}
                        if matches!(&event, Ok(StreamEvent::Error { .. }))
                            || matches!(&event, Err(error) if !matches!(error, GraphError::Interrupted(_)))
                        {
                            producer_services.cancel.cancel();
                        }
                        if graph_events.send(event).await.is_err(){return Ok(());}
                    }
                    (terminal,interrupted)
                };
                let mut paused_checkpoint = None;
                if let Some(event)=interrupted {
                    // Debug streaming publishes only the interrupt's message.
                    // Its committed checkpoint and our durable boundary record
                    // retain the actual identity; no message parsing is involved.
                    let checkpoint=producer_cp.load(&producer_id).await?.context("Interrupted checkpoint absent")?;
                    let boundary=zf_runtime::revisions::load_boundary(
                        producer_services.content_store().as_ref().context("Revision store absent")?,
                        &producer_id,&producer_id,checkpoint.step,scope.trim_end_matches('/'),
                    ).await?;
                    if let Some(boundary)=boundary {
                        let instance=boundary["instance"].as_str().context("Boundary instance absent")?;
                        let previous=definitions.get(instance).context("Boundary instance is not part of this run")?;
                        if boundary["toRevision"].as_str().or(boundary["toHash"].as_str()) != Some(previous.revision().as_str()) {
                            anyhow::ensure!(checkpoint.thread_id==producer_id && checkpoint.pending_nodes.len()==1 && boundary["threadId"]==producer_id && boundary["step"]==checkpoint.step && boundary["node"]==checkpoint.pending_nodes[0],"Revision boundary and committed frontier disagree");
                            let reference=boundary["definitionRef"].as_str().context("Boundary definition absent")?;
                            let definition:zf_runtime::revisions::RevisionDefinition=serde_json::from_value(producer_services.content_store().context("Revision store absent")?.resolve(reference).await?)?;
                            zf_runtime::revisions::validate_definition(&definition)?;
                            anyhow::ensure!(definition.key == previous.key && boundary["toHash"] == definition.hash && boundary["toRevision"].as_str().map_or(definition.package.is_none() && definition.context_selections.is_empty(), |revision| revision == definition.revision()), "Revision boundary definition identity mismatch");
                            anyhow::ensure!(matches!(zf_runtime::revisions::compatibility(&previous.composition,&definition.composition)?,zf_runtime::revisions::Compatibility::SequentialBoundary{..}),"Structural adoption has no proven sequential frontier");
                            let next=if let Some(entry)=&runtime_entry {zf_flows::flow_contract::at_entry(&definition.composition,entry)?}else{definition.composition.clone()};
                            definitions.insert(instance.into(),definition);
                            producer_services.set_revisions(zf_runtime::revisions::RevisionRuntime::new(producer_services.content_store().context("Revision store absent")?,&producer_id,definitions.clone()).await?)?;
                            graph=compiler::build_scope(&next,Some(producer_sender.clone()),scope,Some(producer_services.clone()),Some(Arc::new(producer_cp.clone())))?;
                            producer_sender.send(json!({"type":"flow_revision_adopted","boundary":boundary,"checkpoint":checkpoint.checkpoint_id})).await?;
                            input=State::new();
                            config=config.with_resume_from(&checkpoint.checkpoint_id);
                            continue;
                        }
                    }
                    paused_checkpoint = Some(checkpoint.checkpoint_id);
                    if graph_events.send(event).await.is_err(){return Ok(());}
                }
                if let Some(runtime) = &route_runtime {
                    if launches.as_ref().is_some_and(|task| task.is_finished())
                        && let Some(task) = launches.take()
                    {
                        task.await??;
                    }
                    if launches.is_none() {
                        let runtime = runtime.clone();
                        launches = Some(tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
                            runtime.drain().await.map(|_| ())
                        })));
                    }
                    if let (Some(checkpoint), Some(task)) = (paused_checkpoint, launches.as_mut()) {
                        // Keep the same child owner and event sink alive while
                        // the parent pauses. Dropping a select branch never
                        // cancels the retained drain task.
                        tokio::select! {
                            biased;
                            _ = producer_services.cancel.cancelled() => {
                                task.await??;
                                return Ok(());
                            }
                            Some(request) = resumes.recv() => {
                                anyhow::ensure!(request.checkpoint.as_deref() == Some(checkpoint.as_str()), "Resume checkpoint differs from the active parent frontier");
                                *producer_claim.lock().unwrap_or_else(|p|p.into_inner())=request.claim_id;
                                input = request.input;
                                config = config.with_resume_from(&checkpoint);
                                let _ = request.accepted.send(());
                                continue;
                            }
                            result = &mut *task => { result??; }
                        }
                    } else if let Some(task) = launches.take() {
                        task.await??;
                    }
                }
                if let Some(event)=terminal {let _=graph_events.send(event).await;}
                return Ok(());
            }
        }.await;
        if let Err(error) = result {
            producer_services.cancel.cancel();
            if let Some(runtime) = &route_runtime {
                let _ = runtime.drain().await;
            }
            let _ = graph_events
                .send(Err(GraphError::NodeExecutionFailed {
                    node: "runtime".into(),
                    message: format!("{error:#}"),
                }))
                .await;
        }
    })));
    let mut failure = None;
    let mut pending = Vec::with_capacity(64);
    let mut flush_at = tokio::time::Instant::now() + Duration::from_millis(10);
    loop {
        tokio::select! {
            Some(event) = observed.recv() => {
                apply_observation(&b.content, &mut run, &event).await?;
                pending.push(event);
            }
            item = stream.recv() => {
                let event = match item {
                    Some(Ok(event)) => event,
                    Some(Err(GraphError::Interrupted(_))) => continue,
                    None => break,
                    Some(Err(error)) => {failure=Some(error.to_string());continue;}
                };
                if let StreamEvent::Interrupted { node: interrupted_node, .. } = &event {
                    // Select fairly so continuous child output cannot starve
                    // the parent wait. Its committed checkpoint is an ordered
                    // observation barrier: consume through it before reading
                    // the waiting node, without draining future child traffic.
                    if let Some(header) = cp.storage().latest_header(id).await? {
                        while run["checkpoint"] != header.checkpoint_id {
                            let observed_event = observed.recv().await.context("Parent checkpoint observation absent")?;
                            apply_observation(&b.content, &mut run, &observed_event).await?;
                            pending.push(observed_event);
                            if pending.len() >= 64 { persist_batch(b, id, &mut pending).await?; }
                        }
                    }
                    let expected_path = format!("{scope}{interrupted_node}");
                    let waiting=run["activities"].as_array().into_iter().flatten().rev().find(|a|a["status"]=="waiting" && a["path"]==expected_path).cloned()
                        .or_else(||run["activities"].as_array().into_iter().flatten().rev().find(|a|a["status"]=="waiting" && a["path"].as_str().is_some_and(|path|path.starts_with(scope) && !path[scope.len()..].contains('/'))).cloned());
                    let payload=waiting.as_ref().and_then(|a|wait_payload(&a["interrupt"]).cloned());
                    let node=waiting.as_ref().and_then(|a|a["node"].as_str()).unwrap_or("input").to_owned();
                    let mut config=payload.unwrap_or_else(||doc.nodes.iter().find(|n|n.id==node).map(|n|n.data.config.clone()).unwrap_or(json!({})));
                    for _ in 0..16 {
                        if config["kind"]!="route"{break;}
                        let Some(mut child)=wait_payload(&config["childWait"]).cloned() else{break;};
                        child["routeVisitId"]=config["visitId"].clone();
                        child["routeThreadId"]=config["threadId"].clone();
                        child["routeParentNode"]=config["nodePath"].clone();
                        config=child;
                    }
                    let kind=config["kind"].as_str().unwrap_or("input");
                    let path=config["nodePath"].as_str().unwrap_or(&node);
                    run["status"]=json!(if kind=="stopped" {"stopped"} else {"waiting"});
                    run["runtimeActive"]=json!(live_routes.as_ref().is_some_and(|runtime| runtime.has_active_launches()));
                    let occurrence=run["activities"].as_array().into_iter().flatten().rev().find(|a|a["path"]==path && a["status"]=="waiting").or(waiting.as_ref()).map(|a|a["occurrenceId"].clone()).unwrap_or(Value::Null);
                    run["wait"]=json!({"id":Uuid::new_v4().to_string(),"kind":kind,"node":node,"nodePath":path,"occurrenceId":occurrence,"config":config});
                    run["activeNode"]=json!(path.split('/').next().unwrap_or(path));
                }
                if let StreamEvent::Resumed { .. } = &event {
                    run["status"]=json!("running");
                    run["wait"]=Value::Null;
                    if let Some(activities) = run["activities"].as_array_mut() {
                        for activity in activities.iter_mut().filter(|activity| activity["status"] == "waiting") {
                            activity["status"] = json!("resumed");
                        }
                    }
                }
                if let StreamEvent::Done { .. } = &event {
                    run["status"]=json!("completed");
                    run["wait"]=Value::Null;
                }
                if let StreamEvent::Error {message,..} = &event {failure=Some(message.clone());}

                pending.push(serde_json::to_value(&event)?);
                if matches!(event, StreamEvent::Interrupted { .. }) && run["runtimeActive"] == true {
                    // Persist the parent's actual frontier before waiting for
                    // launched children, so the user can answer immediately.
                    persist_batch(b, id, &mut pending).await?;
                    run["resumeClaimId"]=json!(*current_claim.lock().unwrap_or_else(|p|p.into_inner()));
                    persist_event(b, id, &run, &json!({"type":"run_status","status":run["status"]})).await?;
                    wake_queued_input(b, id).await?;
                }
            }
            _=tokio::time::sleep_until(flush_at), if !pending.is_empty() => {}
        }
        if pending.len() >= 64 || tokio::time::Instant::now() >= flush_at {
            persist_batch(b, id, &mut pending).await?;
            flush_at = tokio::time::Instant::now() + Duration::from_millis(10);
        }
    }
    producer.take().context("Execution producer absent")?.await?;
    while let Ok(event) = observed.try_recv() {
        apply_observation(&b.content, &mut run, &event).await?;
        pending.push(event);
        if pending.len() >= 64 {
            persist_batch(b, id, &mut pending).await?;
        }
    }
    // A cancelled producer can commit a checkpoint before its notification is
    // accepted. Reconcile the latest lightweight header once at this boundary.
    if failure.is_some()
        && let Some(header) = cp.storage().latest_header(id).await?
    {
        let mut event = serde_json::to_value(header)?;
        event["type"] = json!("checkpoint_committed");
        apply_activity(&mut run, &event);
        pending.push(event);
    }
    persist_batch(b, id, &mut pending).await?;
    if let Some(error) = failure {
        let (latest, _) = b.sync.latest(id).await?;
        run["status"] = json!(if latest["abortRequested"] == true {
            "stopped"
        } else if b.closed.load(Ordering::Acquire) {
            "interrupted"
        } else {
            "error"
        });
        run["error"] = json!(error);
        if let Some(activities) = run["activities"].as_array_mut() {
            for activity in activities.iter_mut().filter(|a| a["status"] == "running") {
                activity["status"] = json!("error");
                activity["error"] = json!(error);
            }
        }
    }

    run["runtimeActive"] = json!(false);
    run["activeNodes"] = json!([]);
    // Programmed tools report failures as values, so a cancelled final tool can
    // still lead ADK to Done. Keep the accepted stop distinct from completion.
    if services.cancel.is_cancelled() {
        let (latest,_)=b.sync.latest(id).await?;
        if latest["abortRequested"]==true {run["status"]=json!("stopped");}
        else if b.closed.load(Ordering::Acquire) {run["status"]=json!("interrupted");}
    }
    if run["status"] != "waiting" {
        run["activeNode"] = Value::Null;
    }
    // Publish only a result produced during this invocation, never a stale checkpoint response.
    let fresh: Vec<_> = run["activities"]
        .as_array()
        .into_iter()
        .flatten()
        .skip(previous_activities)
        .filter(|a| a["status"] == "completed" && a["path"] == a["node"])
        .collect();
    let reply = fresh
        .iter()
        .rev()
        .find_map(|a| a["output"]["response"].as_str().map(|text| (*a, text)))
        .or_else(|| {
            fresh
                .iter()
                .rev()
                .find_map(|a| a["output"]["output"].as_str().map(|text| (*a, text)))
        })
        .filter(|(activity, text)| {
            !text.is_empty() && zf_storage::timeline::assistant_message(activity).is_none()
        })
        .map(|(_, text)| text.to_owned());
    if let Some(text) = reply
        && !fresh.iter().any(|a| a["kind"] == "output")
    {
        run["messages"]
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("Messages invalides"))?
            .push(json!({"id":Uuid::new_v4().to_string(),"role":"assistant","text":text}));
    }
    services.clear_sender(&sender);
    run["resumeClaimId"]=json!(*current_claim.lock().unwrap_or_else(|p|p.into_inner()));
    persist_event(
        b,
        id,
        &run,
        &json!({"type":"run_status","status":run["status"]}),
    )
    .await?;
    wake_queued_input(b, id).await
    }.await;
    if result.is_err() {
        // The failed projection must not keep a bounded channel open without a
        // consumer while terminal child receipts try to emit their observations.
        drop(observed);
        cleanup_services.clear_sender(&sender);
        cleanup_services.cancel.cancel();
        if let Some(task) = producer.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(runtime) = cleanup_runtime {
            let _ = runtime.cancel_and_drain().await;
        }
    }
    result
}

async fn wake_queued_input(b: &ExecutionContext, id: &str) -> anyhow::Result<()> {
    // A message can arrive between an inbox's empty check and its checkpoint.
    // Claim a wake-up under the same lock used by message commands.
    let _guard = b.writer.lock().await;
    let mut saved = zf_storage::session_store::load(&b.db, id).await?;
    if saved["status"] == "waiting"
        && saved["wait"]["kind"] != "model_selection"
        && saved["queue"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|m| m["status"] == "pending")
        && run_node_kind(&saved, saved["wait"]["nodePath"].as_str().unwrap_or("")) == Some("inbox")
        && saved["checkpoint"].is_string()
    {
        commands::claim_resume(b, id, &mut saved, State::new()).await?;
    }
    Ok(())
}

fn wait_payload(value: &Value) -> Option<&Value> {
    if value.get("kind").is_some() {
        return Some(value);
    }
    for key in ["Dynamic", "data", "interrupt", "child", "childWait"] {
        if let Some(found) = value.get(key).and_then(wait_payload) {
            return Some(found);
        }
    }
    None
}
pub(crate) async fn services_for(
    b: &ExecutionContext,
    id: &str,
    run: &Value,
) -> anyhow::Result<Arc<RunServices>> {
    let mut services = b.services.lock().await;
    if let Some(current) = services.get(id)
        && !current.cancel.is_cancelled()
    {
        return Ok(current.clone());
    }
    let context: ContextSnapshot = run
        .get("context")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_else(|| b.context.clone());
    let cwd = run["workspacePath"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| context.cwd.clone());
    let current = RunServices::new(
        id.into(),
        cwd,
        b.data.join("runs").join(id),
        context,
        run.get("modelBindings").cloned().unwrap_or(json!({})),
        run["queue"].as_array().cloned().unwrap_or_default(),
    )?;
    current.set_context_sources(
        b.skill_dirs.clone(),
        b.context_home
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from)),
    );
    current.set_content_store(b.content.clone());
    current.set_data_registry(
        zf_storage::data::DataRegistry::new(b.content.pool().clone(), b.content.clone(), id)
            .await?,
    )?;
    current.set_revisions(
        zf_runtime::revisions::RevisionRuntime::new(
            b.content.clone(),
            id,
            revision_definitions(run)?,
        )
        .await?,
    )?;
    if let Some(activations) = run["capabilityActivations"].as_object() {
        for (path, ids) in activations {
            current.set_active_capabilities(path.clone(), serde_json::from_value(ids.clone())?);
        }
    }
    if b.closed.load(Ordering::Acquire) {
        current.cancel.cancel();
    }
    services.insert(id.into(), current.clone());
    Ok(current)
}
