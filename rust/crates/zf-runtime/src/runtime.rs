//! Per-run resources and durable tool receipts. ADK remains the graph executor.
use crate::resources::ResourceReads;
use crate::{
    event_sink::EventSink, workspace_context::ContextSnapshot, workspace_tools::WorkspaceTools,
};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use zf_context::resource_readers::ReaderRegistry;
use zf_storage::content_store::ContentStore;
#[cfg(test)]
use zf_storage::content_store::decode_full_output;

tokio::task_local! {
    pub static CURRENT_OCCURRENCE: (String, String);
}

pub fn current_origin() -> Option<Value> {
    CURRENT_OCCURRENCE
        .try_with(
            |(node_path, occurrence_id)| json!({"nodePath":node_path,"occurrenceId":occurrence_id}),
        )
        .ok()
}

/// Capabilities supplied by an explicitly resolved bridge. Invocation receives
/// the durable effect identity, after the caller's provenance has been checked.
#[async_trait::async_trait]
pub trait DynamicCapabilities: Send + Sync {
    fn tools(&self, agent_path: &str) -> Vec<Value>;
    fn resume_channels(&self) -> Vec<String> {
        vec![]
    }
    /// Inspect an explicitly declared route without publishing data or invoking
    /// its target. Producers use this immutable descriptor in their fingerprint.
    async fn route_contract(
        &self,
        _path: &str,
        _branch: &str,
        _route_id: Option<&str>,
    ) -> Result<Value> {
        bail!("No composed route runtime is installed")
    }
    async fn await_visit(
        &self,
        _path: &str,
        _visit_id: &str,
        _state: &adk_graph::State,
    ) -> Result<RouteOutcome> {
        bail!("No composed route runtime is installed")
    }
    async fn capture(
        &self,
        _agent_path: &str,
        _invocation_id: &str,
        _state: &adk_graph::State,
    ) -> Result<()> {
        Ok(())
    }
    async fn resume_state(
        &self,
        _agent_path: &str,
        _call_id: &str,
        _state: &adk_graph::State,
    ) -> Result<()> {
        Ok(())
    }
    async fn invoke_branch(&self, _invocation: BranchInvocation) -> Result<RouteOutcome> {
        bail!("No composed route runtime is installed")
    }
    /// Only a host with a durable invocation manifest may recover an interrupted
    /// routed graph. Ordinary tools deliberately keep the default refusal.
    async fn recover_started(
        &self,
        _agent_path: &str,
        _call_id: &str,
        _name: &str,
        _arguments: &Value,
    ) -> Result<Option<Value>> {
        Ok(None)
    }
    async fn invoke(
        &self,
        agent_path: &str,
        call_id: &str,
        name: &str,
        arguments: Value,
    ) -> Result<Value>;
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInvocation {
    pub path: String,
    pub branch: String,
    pub invocation: zf_flows::composition::InvocationKind,
    pub route_id: Option<String>,
    pub call_id: String,
    pub input: Value,
    pub caller_state: adk_graph::State,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RouteOutcome {
    Skipped {
        reason: String,
    },
    Completed {
        visit_id: String,
        thread_id: String,
        result: Value,
    },
    Launched {
        visit_id: String,
        thread_id: String,
    },
    Waiting {
        visit_id: String,
        thread_id: String,
        wait: Value,
    },
    Handoff {
        visit_id: String,
        thread_id: String,
        result: Value,
    },
}
impl RouteOutcome {
    pub fn marker(&self) -> Value {
        let result = match self {
            Self::Completed { result, .. } | Self::Handoff { result, .. } => result.clone(),
            _ => Value::Null,
        };
        json!({"__zedflowRoute":self,"result":result})
    }
}

pub struct RunServices {
    pub id: String,
    pub cwd: PathBuf,
    pub data: PathBuf,
    pub context: Arc<ContextSnapshot>,
    pub tools: WorkspaceTools,
    pub cancel: CancellationToken,
    bindings: Arc<RwLock<BTreeMap<String, Value>>>,
    activations: Arc<RwLock<BTreeMap<String, Vec<String>>>>,
    queue: Arc<RwLock<Vec<Value>>>,
    inbox_commands: Arc<tokio::sync::RwLock<()>>,
    sender: Arc<RwLock<Option<EventSink>>>,
    // Receipt claims serialize each effect identity. Distinct native graph
    // branches may execute concurrently; export takes the exclusive barrier.
    journal: Arc<tokio::sync::RwLock<()>>,
    context_sources: Arc<RwLock<(Vec<PathBuf>, Option<PathBuf>)>>,
    directory_contexts: Arc<RwLock<BTreeMap<PathBuf, Arc<ContextSnapshot>>>>,
    store: Arc<RwLock<Option<ContentStore>>>,
    registry: Arc<RwLock<Option<zf_storage::data::DataRegistry>>>,
    dynamic_capabilities: Arc<RwLock<Option<Arc<dyn DynamicCapabilities>>>>,
    revisions: Arc<RwLock<Option<Arc<crate::revisions::RevisionRuntime>>>>,
    reader_host: Arc<RwLock<ResourceReads>>,
    resource_readers: Arc<RwLock<Option<Arc<ReaderRegistry>>>>,
}

/// Excludes inbox claims until a cancellation has durably committed or failed.
/// Dropping an uncommitted reservation leaves the queue unchanged.
pub struct MessageCancellation {
    queue: Arc<RwLock<Vec<Value>>>,
    id: String,
    _guard: tokio::sync::OwnedRwLockWriteGuard<()>,
}
impl MessageCancellation {
    /// Publish a cancellation only after the caller's durable command commits.
    pub fn commit(self) {
        let mut queue = self.queue.write().unwrap_or_else(|p| p.into_inner());
        if let Some(message) = queue.iter_mut().find(|message| message["id"] == self.id) {
            message["status"] = json!("cancelled");
        }
    }
}

impl RunServices {
    pub async fn local(cwd: PathBuf, data: PathBuf) -> Result<Arc<Self>> {
        let context = ContextSnapshot::load(&cwd, &[]).await?;
        tokio::fs::create_dir_all(&data).await?;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(data.join("content.db"))
                    .create_if_missing(true)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
            )
            .await?;
        let store = ContentStore::new(pool).await?;
        let services = Self::new(
            uuid::Uuid::new_v4().to_string(),
            cwd,
            data,
            context,
            json!({}),
            vec![],
        )?;
        let registry =
            zf_storage::data::DataRegistry::new(store.pool().clone(), store.clone(), &services.id)
                .await?;
        services.set_content_store(store);
        services.set_data_registry(registry)?;
        Ok(services)
    }

    pub fn new(
        id: String,
        cwd: PathBuf,
        data: PathBuf,
        context: ContextSnapshot,
        bindings: Value,
        queue: Vec<Value>,
    ) -> Result<Arc<Self>> {
        let cancel = CancellationToken::new();
        let reader_host = Arc::new(RwLock::new(ResourceReads::new(None, cancel.clone())));
        Ok(Arc::new(Self {
            tools: WorkspaceTools::new(cwd.clone(), data.clone())?,
            id,
            cwd,
            data,
            context: Arc::new(context),
            cancel,
            bindings: Arc::new(RwLock::new(
                serde_json::from_value(bindings).context("invalid model bindings")?,
            )),
            activations: Arc::new(RwLock::new(BTreeMap::new())),
            queue: Arc::new(RwLock::new(queue)),
            inbox_commands: Arc::new(tokio::sync::RwLock::new(())),
            sender: Arc::new(RwLock::new(None)),
            journal: Arc::new(tokio::sync::RwLock::new(())),
            context_sources: Arc::new(RwLock::new((
                vec![],
                std::env::var_os("HOME").map(PathBuf::from),
            ))),
            directory_contexts: Arc::new(RwLock::new(BTreeMap::new())),
            store: Arc::new(RwLock::new(None)),
            registry: Arc::new(RwLock::new(None)),
            dynamic_capabilities: Arc::new(RwLock::new(None)),
            revisions: Arc::new(RwLock::new(None)),
            reader_host,
            resource_readers: Arc::new(RwLock::new(None)),
        }))
    }

    /// Scope only filesystem/context access. Receipt claims, events, model choices,
    /// cancellation and entity storage remain shared across every flow instance.
    pub fn for_composition(
        self: &Arc<Self>,
        doc: &zf_flows::schema::Composition,
        _scope: &str,
    ) -> Result<Arc<Self>> {
        self.for_working_directory(doc.settings.working_directory.as_deref())
    }
    pub fn for_working_directory(self: &Arc<Self>, directory: Option<&str>) -> Result<Arc<Self>> {
        let Some(directory) = directory else {
            return Ok(self.clone());
        };
        ensure!(
            !directory.trim().is_empty(),
            "Le répertoire de travail du flow est vide"
        );
        let path = PathBuf::from(directory);
        let cwd = std::fs::canonicalize(if path.is_absolute() {
            path
        } else {
            self.cwd.join(path)
        })
        .with_context(|| format!("Répertoire de travail du flow inaccessible : {directory}"))?;
        ensure!(
            cwd.is_dir(),
            "Le répertoire de travail du flow doit être un dossier : {}",
            cwd.display()
        );
        if cwd == self.cwd {
            return Ok(self.clone());
        }
        let cached = self
            .directory_contexts
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&cwd)
            .cloned();
        let context = if let Some(context) = cached {
            context
        } else {
            let (dirs, home) = self
                .context_sources
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            let context = Arc::new(crate::workspace_context::load_snapshot(
                &cwd,
                &dirs,
                home.as_deref(),
            )?);
            self.directory_contexts
                .write()
                .unwrap_or_else(|p| p.into_inner())
                .entry(cwd.clone())
                .or_insert_with(|| context.clone())
                .clone()
        };
        Ok(Arc::new(Self {
            id: self.id.clone(),
            tools: WorkspaceTools::new(cwd.clone(), self.data.clone())?,
            cwd,
            data: self.data.clone(),
            context,
            cancel: self.cancel.clone(),
            bindings: self.bindings.clone(),
            activations: self.activations.clone(),
            queue: self.queue.clone(),
            inbox_commands: self.inbox_commands.clone(),
            sender: self.sender.clone(),
            journal: self.journal.clone(),
            store: self.store.clone(),
            registry: self.registry.clone(),
            dynamic_capabilities: self.dynamic_capabilities.clone(),
            revisions: self.revisions.clone(),
            reader_host: self.reader_host.clone(),
            resource_readers: self.resource_readers.clone(),
            context_sources: self.context_sources.clone(),
            directory_contexts: self.directory_contexts.clone(),
        }))
    }
    /// Validate a chosen runtime's filesystem locations without changing process cwd.
    pub async fn validate_directories(
        doc: &zf_flows::schema::Composition,
        workspace: &std::path::Path,
    ) -> Result<()> {
        let mut pending = vec![(doc.clone(), workspace.to_owned())];
        while let Some((flow, base)) = pending.pop() {
            let cwd = if let Some(directory) = &flow.settings.working_directory {
                ensure!(
                    !directory.trim().is_empty(),
                    "Le répertoire de travail du flow est vide"
                );
                let path = PathBuf::from(directory);
                let path = if path.is_absolute() {
                    path
                } else {
                    base.join(path)
                };
                let resolved = tokio::fs::canonicalize(&path).await.with_context(|| {
                    format!(
                        "Répertoire du flow {} inaccessible : {}",
                        flow.name,
                        path.display()
                    )
                })?;
                ensure!(
                    tokio::fs::metadata(&resolved).await?.is_dir(),
                    "Le répertoire du flow {} doit être un dossier",
                    flow.name
                );
                resolved
            } else {
                base
            };
            for node in &flow.nodes {
                if node.data.kind == "subgraph" {
                    pending.push((
                        serde_json::from_value(node.data.config["composition"].clone())?,
                        cwd.clone(),
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn set_context_sources(&self, directories: Vec<PathBuf>, home: Option<PathBuf>) {
        *self
            .context_sources
            .write()
            .unwrap_or_else(|p| p.into_inner()) = (directories, home);
    }

    pub fn set_content_store(&self, store: ContentStore) {
        *self.reader_host.write().unwrap_or_else(|p| p.into_inner()) =
            ResourceReads::new(Some(store.clone()), self.cancel.clone());
        *self.store.write().unwrap_or_else(|p| p.into_inner()) = Some(store);
    }
    pub fn resource_reads(&self) -> ResourceReads {
        self.reader_host
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn resource_readers(&self) -> Result<Arc<ReaderRegistry>> {
        if let Some(readers) = self
            .resource_readers
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            return Ok(readers);
        }
        Ok(Arc::new(
            self.resource_reads().native_readers(self.cwd.clone())?,
        ))
    }
    pub fn set_resource_readers(&self, readers: Arc<ReaderRegistry>) {
        *self
            .resource_readers
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Some(readers);
    }
    pub fn content_store(&self) -> Option<ContentStore> {
        self.store.read().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn set_data_registry(&self, registry: zf_storage::data::DataRegistry) -> Result<()> {
        ensure!(
            registry.universe() == self.id,
            "Registry belongs to a different runtime universe"
        );
        *self.registry.write().unwrap_or_else(|p| p.into_inner()) = Some(registry);
        Ok(())
    }
    pub fn data_registry(&self) -> Option<zf_storage::data::DataRegistry> {
        self.registry
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn set_dynamic_capabilities(&self, capabilities: Arc<dyn DynamicCapabilities>) {
        *self
            .dynamic_capabilities
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Some(capabilities);
    }
    pub fn dynamic_capabilities(&self) -> Option<Arc<dyn DynamicCapabilities>> {
        self.dynamic_capabilities
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    /// Immutable runtime records. CAS is the authority when attached; legacy
    /// files are consulted only to resume an unmigrated, already existing record.
    pub async fn read_record(&self, kind: &str, key: &str) -> Result<Option<Value>> {
        ensure!(
            !key.contains('/') && !key.contains('\\') && !key.starts_with('.'),
            "invalid runtime record key"
        );
        if let Some(store) = self.content_store()
            && let Some(value) = store.record(&self.id, kind, key).await?
        {
            return Ok(Some(value));
        }
        let file = self.data.join(kind).join(format!("{key}.json"));
        match tokio::fs::read(file).await {
            Ok(raw) => Ok(Some(serde_json::from_slice(&raw)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub async fn persist_record(
        &self,
        kind: &str,
        key: &str,
        value: &Value,
    ) -> Result<Option<String>> {
        if let Some(store) = self.content_store() {
            if !store.claim_record(&self.id, kind, key, value).await? {
                ensure!(
                    store.record(&self.id, kind, key).await?.as_ref() == Some(value),
                    "runtime record identity reused"
                );
            }
            return Ok(Some(store.intern(value).await?));
        }
        let root = self.data.join(kind);
        tokio::fs::create_dir_all(&root).await?;
        let temporary = root.join(format!(".{key}.{}.pending", uuid::Uuid::new_v4()));
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(&serde_json::to_vec(value)?).await?;
        file.sync_all().await?;
        tokio::fs::hard_link(&temporary, root.join(format!("{key}.json"))).await?;
        tokio::fs::remove_file(temporary).await?;
        sync_directory(&root).await?;
        Ok(None)
    }

    pub fn binding(&self, path: &str) -> Option<Value> {
        self.bindings
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(path)
            .cloned()
    }

    pub fn set_binding(&self, path: String, selection: Value) {
        self.bindings
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path, selection);
    }

    pub fn active_capabilities(&self, path: &str) -> Vec<String> {
        self.activations
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(path)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_active_capabilities(&self, path: String, ids: Vec<String>) {
        self.activations
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path, ids);
    }

    /// Export holds the application writer and then this guard after checking
    /// that the run is quiescent, so all durable tool files form one snapshot.
    pub async fn export_barrier(&self) -> tokio::sync::RwLockWriteGuard<'_, ()> {
        self.journal.write().await
    }

    pub fn replace_queue(&self, mut queue: Vec<Value>) {
        let mut current = self.queue.write().unwrap_or_else(|p| p.into_inner());
        for message in &mut queue {
            if message["status"] == "pending"
                && current.iter().any(|existing| {
                    existing["id"] == message["id"] && existing["status"] == "claimed"
                })
            {
                message["status"] = json!("claimed");
            }
        }
        *current = queue;
    }

    pub fn pending_message(&self, kind: &str, consumed: &[String]) -> Option<Value> {
        self.queue
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|message| {
                message["kind"] == kind
                    && message["status"] == "pending"
                    && message["id"]
                        .as_str()
                        .is_some_and(|id| !consumed.iter().any(|seen| seen == id))
            })
            .cloned()
    }

    /// Prevent a concurrent client removal after the graph has selected a message.
    /// This is an in-memory claim only: the graph checkpoint's consumed IDs remain
    /// authoritative, so an uncheckpointed claim can be retried after restart.
    pub async fn claim_message(&self, kind: &str, consumed: &[String]) -> Option<Value> {
        let _guard = self.inbox_commands.read().await;
        let mut queue = self.queue.write().unwrap_or_else(|p| p.into_inner());
        let message = queue.iter_mut().find(|message| {
            message["kind"] == kind
                && message["status"] == "pending"
                && message["id"]
                    .as_str()
                    .is_some_and(|id| !consumed.iter().any(|seen| seen == id))
        })?;
        message["status"] = json!("claimed");
        Some(message.clone())
    }

    /// Reserve a pending message without changing runtime-visible queue state.
    /// The caller must retain this guard through its durable cancellation write.
    pub async fn reserve_message_cancellation(&self, id: &str) -> Result<MessageCancellation> {
        let guard = self.inbox_commands.clone().write_owned().await;
        {
            let queue = self.queue.read().unwrap_or_else(|p| p.into_inner());
            let message = queue
                .iter()
                .find(|message| message["id"] == id)
                .context("message not found")?;
            ensure!(
                message["status"] == "pending",
                "message has already been claimed or processed"
            );
        }
        Ok(MessageCancellation {
            queue: self.queue.clone(),
            id: id.into(),
            _guard: guard,
        })
    }

    pub async fn cancel_message(&self, id: &str) -> Result<()> {
        self.reserve_message_cancellation(id).await?.commit();
        Ok(())
    }

    pub fn set_revisions(&self, revisions: Arc<crate::revisions::RevisionRuntime>) -> Result<()> {
        ensure!(
            revisions.run_id() == self.id,
            "Revision runtime belongs to another run"
        );
        *self.revisions.write().unwrap_or_else(|p| p.into_inner()) = Some(revisions);
        Ok(())
    }
    pub fn revisions(&self) -> Option<Arc<crate::revisions::RevisionRuntime>> {
        self.revisions
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn set_sender(&self, sender: Option<EventSink>) {
        *self.sender.write().unwrap_or_else(|p| p.into_inner()) = sender;
    }

    pub fn clear_sender(&self, completed: &EventSink) {
        let mut sender = self.sender.write().unwrap_or_else(|p| p.into_inner());
        if sender
            .as_ref()
            .is_some_and(|current| current.same_channel(completed))
        {
            *sender = None;
        }
    }

    pub async fn emit(&self, mut event: Value) {
        if event.get("flowRevision").is_none()
            && let Some(revision) = crate::revisions::current_revision()
        {
            event["flowRevision"] = revision;
        }
        if event.get("origin").is_none()
            && let Some(origin) = current_origin()
        {
            event["origin"] = origin;
        }
        let sender = self
            .sender
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(sender) = sender {
            let _ = sender.send(event).await;
        }
    }

    pub fn system_prompt(&self) -> String {
        self.context.system_prompt()
    }

    pub fn system_prompt_for_tools(&self, tools: &[String]) -> String {
        self.context.system_prompt_for_tools(tools)
    }

    pub async fn execute_tool(
        &self,
        path: &str,
        call_id: &str,
        name: &str,
        args: Value,
    ) -> Result<Value> {
        let guard = tokio::select! {
            biased;
            () = self.cancel.cancelled() => bail!("operation cancelled before execution"),
            guard = self.journal.read() => guard,
        };
        ensure!(
            !self.cancel.is_cancelled(),
            "operation cancelled before execution"
        );
        let key = format!(
            "{:x}",
            Sha256::digest(format!("{}\0{path}\0{call_id}", self.id))
        );
        let store = self.content_store();
        let receipts = self.data.join("receipts");
        let file = receipts.join(format!("{key}.json"));
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let mut receipt = json!({"id":call_id,"nodePath":path,"name":name,"arguments":args,"status":"started","startedAt":started_at});
        let claimed = if let Some(store) = &store {
            // Import a pre-existing filesystem receipt before attempting the CAS
            // claim. An unfinished old call must never become a new effect.
            if let Some(saved) = self.read_record("receipts", &key).await? {
                store
                    .claim_record(&self.id, "receipts", &key, &saved)
                    .await?;
                false
            } else {
                store
                    .claim_record(&self.id, "receipts", &key, &receipt)
                    .await?
            }
        } else {
            tokio::fs::create_dir_all(&receipts).await?;
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)
                .await
            {
                Ok(mut handle) => {
                    handle.write_all(&serde_json::to_vec(&receipt)?).await?;
                    handle.sync_all().await?;
                    sync_directory(&receipts).await?;
                    true
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error).context("cannot claim tool receipt"),
            }
        };
        // A routed child must be able to execute its own workspace tools. Its
        // durable receipt is already claimed; the route host serializes visits.
        let _ordinary_tool_guard = if !crate::operations::tool_declarations().contains_key(name)
            && self.dynamic_capabilities().is_some()
        {
            drop(guard);
            None
        } else {
            Some(guard)
        };
        let mut recovered = None;
        if !claimed {
            let saved = self
                .read_record("receipts", &key)
                .await?
                .context("claimed receipt is missing")?;
            ensure!(
                saved["name"] == name && saved["arguments"] == args,
                "tool receipt identity reused with different arguments"
            );
            if saved["status"] == "completed" {
                if name == "read" {
                    self.observe_skill_read(&saved["result"]).await;
                }
                return Ok(saved["result"].clone());
            }
            if saved["status"] == "failed" {
                anyhow::bail!("{}", saved["error"].as_str().unwrap_or("tool failed"));
            }
            if zf_context::window::is_tool(name) {
                recovered = Some(
                    crate::resources::window::execute_tool(self, path, call_id, name, &args)
                        .await?,
                );
            } else if !crate::operations::tool_declarations().contains_key(name)
                && let Some(capabilities) = self.dynamic_capabilities()
            {
                recovered = capabilities
                    .recover_started(path, call_id, name, &args)
                    .await?;
            }
            ensure!(
                recovered.is_some(),
                "Appel interrompu : effet inconnu. Inspecter le résultat avant de proposer une nouvelle action ; cet appel ne sera pas rejoué automatiquement."
            );
        }
        let sender = self
            .sender
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let node_path = path.to_owned();
        let event_id = call_id.to_owned();
        let tool_name = name.to_owned();
        let origin = current_origin();
        let output_ref = Arc::new(RwLock::new(None::<String>));
        let output_sink = if name == "exec" {
            if let Some(store) = &store {
                ensure!(
                    store
                        .claim_record(&self.id, "tool-output-fragments", &key, &json!([]))
                        .await?,
                    "tool output identity already exists"
                );
                let empty = store.intern(&json!([])).await?;
                *output_ref.write().unwrap_or_else(|p| p.into_inner()) = Some(empty);
                let store = store.clone();
                let scope = self.id.clone();
                let record_key = key.clone();
                let captured = output_ref.clone();
                let index = Arc::new(std::sync::atomic::AtomicU64::new(0));
                Some(Arc::new(move |event: crate::workspace_tools::OutputEvent| {
                    let store = store.clone();
                    let scope = scope.clone();
                    let record_key = record_key.clone();
                    let captured = captured.clone();
                    let index = index.clone();
                    Box::pin(async move {
                        let (stream, bytes) = match event {
                            crate::workspace_tools::OutputEvent::Opened(path) => {
                                store
                                    .put_record(
                                        &scope,
                                        "tool-output",
                                        &record_key,
                                        &json!({"path":path,"fragmentKey":record_key}),
                                    )
                                    .await?;
                                return Ok(());
                            }
                            crate::workspace_tools::OutputEvent::Bytes { stream, bytes } => {
                                (stream, bytes)
                            }
                        };
                        let index = index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let item = json!({"index":index,"stream":stream,"data":base64::engine::general_purpose::STANDARD.encode(bytes)});
                        let reference = store
                            .append_record_item(&scope, "tool-output-fragments", &record_key, &item)
                            .await?;
                        *captured.write().unwrap_or_else(|p| p.into_inner()) = Some(reference);
                        Ok(())
                    }) as futures::future::BoxFuture<'static, Result<()>>
                }) as crate::workspace_tools::OutputSink)
            } else {
                None
            }
        } else {
            None
        };
        let captured_ref = output_ref.clone();
        let progress = Arc::new(move |mut event: Value| {
            if let Some(reference) = captured_ref
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
            {
                event["fullOutputRef"] = json!(reference);
            }
            event["type"] = json!("tool_progress");
            event["nodePath"] = json!(node_path);
            event["callId"] = json!(event_id);
            event["name"] = json!(tool_name);
            if let Some(origin) = &origin {
                event["origin"] = origin.clone();
            }
            if let Some(sender) = &sender {
                sender.emit(event);
            }
        });
        self.emit(json!({"type":"tool_call","nodePath":path,"callId":call_id,"name":name,"arguments":args,"startedAt":started_at})).await;
        let execution_started = std::time::Instant::now();
        let mut result = if let Some(result) = recovered {
            Ok(result)
        } else if crate::operations::tool_declarations().contains_key(name) {
            self.tools
                .execute_with_output_sink(
                    name,
                    args,
                    call_id,
                    self.cancel.clone(),
                    progress,
                    output_sink,
                )
                .await
        } else if zf_context::window::is_tool(name) {
            crate::resources::window::execute_tool(self, path, call_id, name, &args).await
        } else if let Some(capabilities) = self.dynamic_capabilities() {
            if capabilities
                .tools(path)
                .iter()
                .any(|tool| tool["name"] == name)
            {
                capabilities.invoke(path, call_id, name, args).await
            } else {
                Err(anyhow::anyhow!(
                    "Capability is not exposed to this agent: {name}"
                ))
            }
        } else {
            Err(anyhow::anyhow!("No adapter for capability: {name}"))
        };
        if let Some(reference) = output_ref.read().unwrap_or_else(|p| p.into_inner()).clone() {
            receipt["fullOutputRef"] = json!(reference);
            if let Ok(value) = &mut result {
                value["fullOutputRef"] = json!(reference);
            }
        }
        match &result {
            Ok(value) => {
                receipt["status"] = json!(if value["__zedflowRoute"]["status"] == "waiting" {
                    "waiting"
                } else {
                    "completed"
                });
                receipt["result"] = value.clone();
                if name == "read" {
                    self.observe_skill_read(value).await;
                }
            }
            Err(error) => {
                receipt["status"] = json!("failed");
                receipt["error"] = json!(format!("{error:#}"));
            }
        }
        receipt["durationMs"] = json!(execution_started.elapsed().as_millis());
        receipt["endedAt"] = json!(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default()
        );
        let receipt_ref = if let Some(store) = &store {
            Some(
                store
                    .put_record(&self.id, "receipts", &key, &receipt)
                    .await?,
            )
        } else {
            write_document(&file, &receipt).await?;
            None
        };
        self.emit(json!({"type":"tool_result","nodePath":path,"callId":call_id,"name":name,"result":receipt.get("result"),"error":receipt.get("error"),"status":receipt["status"],"startedAt":started_at,"endedAt":receipt["endedAt"],"durationMs":receipt["durationMs"],"receiptRef":receipt_ref,"fullOutputRef":receipt.get("fullOutputRef")})).await;
        result
    }

    async fn observe_skill_read(&self, result: &Value) {
        let (Some(path), Some(content)) = (result["path"].as_str(), result["content"].as_str())
        else {
            return;
        };
        let Ok(path) = tokio::fs::canonicalize(path).await else {
            return;
        };
        if let Some(skill) = self.context.skills.iter().find(|skill| skill.path == path) {
            self.emit(json!({"type":"skill_loaded","name":skill.name,"path":path,"hash":format!("{:x}",Sha256::digest(content.as_bytes())),"source":"read","truncated":result["truncated"]})).await;
        }
    }
}

async fn write_document(path: &std::path::Path, value: &Value) -> Result<()> {
    let temporary = path.with_extension(format!("{}.pending", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await?;
    file.write_all(&serde_json::to_vec(value)?).await?;
    file.sync_all().await?;
    tokio::fs::rename(&temporary, path).await?;
    sync_directory(path.parent().context("receipt has no parent")?).await?;
    Ok(())
}

async fn sync_directory(directory: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    tokio::fs::File::open(directory).await?.sync_all().await?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn services(directory: &std::path::Path, queue: Vec<Value>) -> Arc<RunServices> {
        RunServices::new(
            "test-run".into(),
            directory.to_path_buf(),
            directory.join("data"),
            ContextSnapshot::default(),
            json!({}),
            queue,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn output_fragments_are_durable_before_tool_completion_and_survive_aborted_future() {
        let directory = tempfile::tempdir().unwrap();
        let store = ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        let service = services(directory.path(), vec![]);
        service.set_content_store(store.clone());
        let worker = service.clone();
        let args = json!({"command":"printf x >> effect; printf first; sleep 10; printf last"});
        let worker_args = args.clone();
        let task = tokio::spawn(async move {
            worker
                .execute_tool("tools", "partial", "exec", worker_args)
                .await
        });
        let key = format!("{:x}", Sha256::digest("test-run\0tools\0partial"));
        let fragments = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if let Some(value) = store
                    .record("test-run", "tool-output-fragments", &key)
                    .await
                    .unwrap()
                    && value.as_array().is_some_and(|items| !items.is_empty())
                {
                    break value;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(!task.is_finished());
        assert_eq!(decode_full_output(&fragments).unwrap(), b"first");
        assert_eq!(fragments[0]["stream"], "stdout");
        assert_eq!(
            store
                .record("test-run", "receipts", &key)
                .await
                .unwrap()
                .unwrap()["status"],
            "started"
        );
        assert_eq!(
            store
                .record("test-run", "tool-output", &key)
                .await
                .unwrap()
                .unwrap()["fragmentKey"],
            key
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let restarted = services(directory.path(), vec![]);
        restarted.set_content_store(store.clone());
        assert!(
            restarted
                .execute_tool("tools", "partial", "exec", args)
                .await
                .unwrap_err()
                .to_string()
                .contains("effet inconnu")
        );
        assert_eq!(
            tokio::fs::read_to_string(directory.path().join("effect"))
                .await
                .unwrap(),
            "x"
        );
        assert_eq!(
            decode_full_output(
                &store
                    .record("test-run", "tool-output-fragments", &key)
                    .await
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            b"first"
        );
    }
    #[tokio::test]
    async fn cas_receipts_survive_restart_and_keep_binary_output_without_files() {
        let directory = tempfile::tempdir().unwrap();
        let store = ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        let first = services(directory.path(), vec![]);
        first.set_content_store(store.clone());
        let args = json!({"command":"printf x >> once; printf '\\377\\000hello'"});
        let result = first
            .execute_tool("tools", "cas-once", "exec", args.clone())
            .await
            .unwrap();
        let output_ref = result["fullOutputRef"].as_str().unwrap();
        let output = store.resolve(output_ref).await.unwrap();
        let bytes = decode_full_output(&output).unwrap();
        assert_eq!(bytes, b"\xff\x00hello");
        let log = result["fullOutputPath"].as_str().unwrap();
        tokio::fs::remove_file(log).await.unwrap();
        assert!(
            !directory.path().join("data/receipts").exists(),
            "new receipt must only live in CAS"
        );
        let restarted = services(directory.path(), vec![]);
        restarted.set_content_store(store.clone());
        assert_eq!(
            restarted
                .execute_tool("tools", "cas-once", "exec", args)
                .await
                .unwrap(),
            result
        );
        assert_eq!(
            tokio::fs::read_to_string(directory.path().join("once"))
                .await
                .unwrap(),
            "x"
        );
        assert!(
            store
                .records("test-run")
                .await
                .unwrap()
                .iter()
                .any(|record| record.kind == "tool-output-fragments")
        );
        let key = format!("{:x}", Sha256::digest("test-run\0tools\0unknown"));
        let args = json!({"command":"touch must-not-exist"});
        store.claim_record("test-run","receipts",&key,&json!({"id":"unknown","nodePath":"tools","name":"exec","arguments":args,"status":"started"})).await.unwrap();
        assert!(
            restarted
                .execute_tool("tools", "unknown", "exec", args)
                .await
                .unwrap_err()
                .to_string()
                .contains("effet inconnu")
        );
        assert!(!directory.path().join("must-not-exist").exists());
    }
    #[tokio::test]
    async fn previous_invocation_cannot_clear_next_invocation_sender() {
        let directory = tempfile::tempdir().unwrap();
        let service = services(directory.path(), vec![]);
        let (previous, _previous_events) = crate::event_sink::channel(64);
        let (next, mut next_events) = crate::event_sink::channel(64);
        service.set_sender(Some(previous.clone()));
        service.set_sender(Some(next.clone()));
        service.clear_sender(&previous);
        service.emit(json!({"invocation":"next"})).await;
        assert_eq!(
            next_events.try_recv().unwrap(),
            json!({"invocation":"next"})
        );
        service.clear_sender(&next);
        service.emit(json!({"invocation":"completed"})).await;
        assert!(next_events.try_recv().is_err());
    }

    #[tokio::test]
    async fn completed_and_failed_receipts_never_repeat_effects() {
        let directory = tempfile::tempdir().unwrap();
        let service = services(directory.path(), vec![]);
        let args = json!({"command":"printf x >> successful"});
        let first = service
            .execute_tool("tools", "success", "exec", args.clone())
            .await
            .unwrap();
        let second = service
            .execute_tool("tools", "success", "exec", args)
            .await
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            std::fs::read_to_string(directory.path().join("successful")).unwrap(),
            "x"
        );
        let args = json!({"command":"printf y >> failed; exit 7"});
        assert!(
            service
                .execute_tool("tools", "failure", "exec", args.clone())
                .await
                .is_err()
        );
        assert!(
            service
                .execute_tool("tools", "failure", "exec", args)
                .await
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("failed")).unwrap(),
            "y"
        );
        assert!(
            service
                .execute_tool(
                    "tools",
                    "success",
                    "exec",
                    json!({"command":"touch forbidden"})
                )
                .await
                .is_err()
        );
        assert!(!directory.path().join("forbidden").exists());
    }

    #[tokio::test]
    async fn started_receipt_blocks_replay_after_restart() {
        let directory = tempfile::tempdir().unwrap();
        let receipts = directory.path().join("data/receipts");
        std::fs::create_dir_all(&receipts).unwrap();
        let key = format!("{:x}", Sha256::digest("test-run\0tools\0uncertain"));
        let args = json!({"command":"touch repeated"});
        std::fs::write(receipts.join(format!("{key}.json")), serde_json::to_vec(&json!({"id":"uncertain","nodePath":"tools","name":"exec","arguments":args,"status":"started"})).unwrap()).unwrap();
        let service = services(directory.path(), vec![]);
        let error = service
            .execute_tool("tools", "uncertain", "exec", args)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("effet inconnu"));
        assert!(!directory.path().join("repeated").exists());
    }

    #[tokio::test]
    async fn cancellation_before_claim_creates_no_effect_or_receipt() {
        let directory = tempfile::tempdir().unwrap();
        let service = services(directory.path(), vec![]);
        service.cancel.cancel();
        assert!(
            service
                .execute_tool(
                    "tools",
                    "cancelled",
                    "exec",
                    json!({"command":"touch forbidden"})
                )
                .await
                .is_err()
        );
        assert!(!directory.path().join("forbidden").exists());
        assert!(!directory.path().join("data/receipts").exists());
    }

    #[tokio::test]
    async fn separate_service_instances_cannot_claim_the_same_effect() {
        let directory = tempfile::tempdir().unwrap();
        let first = services(directory.path(), vec![]);
        let second = services(directory.path(), vec![]);
        let args = json!({"command":"printf x >> once; sleep 0.03"});
        let (one, two) = tokio::join!(
            first.execute_tool("tools", "shared", "exec", args.clone()),
            second.execute_tool("tools", "shared", "exec", args)
        );
        assert!(one.is_ok() || two.is_ok());
        assert_eq!(
            std::fs::read_to_string(directory.path().join("once")).unwrap(),
            "x"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn claim_and_cancellation_have_one_winner_and_checkpoint_remains_authority() {
        let directory = tempfile::tempdir().unwrap();
        let queue =
            vec![json!({"id":"message","kind":"steering","text":"instruction","status":"pending"})];
        let service = services(directory.path(), queue.clone());
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let claim = {
            let (service, barrier) = (service.clone(), barrier.clone());
            tokio::spawn(async move {
                barrier.wait().await;
                service.claim_message("steering", &[]).await.is_some()
            })
        };
        let cancel = {
            let (service, barrier) = (service.clone(), barrier.clone());
            tokio::spawn(async move {
                barrier.wait().await;
                service.cancel_message("message").await.is_ok()
            })
        };
        barrier.wait().await;
        let (claimed, cancelled) = (claim.await.unwrap(), cancel.await.unwrap());
        assert_ne!(claimed, cancelled);
        assert!(service.pending_message("steering", &[]).is_none());
        if claimed {
            service.replace_queue(queue.clone());
            assert!(
                service.claim_message("steering", &[]).await.is_none(),
                "old DB snapshot cannot unclaim delivery"
            );
            let recovered = services(directory.path(), queue);
            assert!(
                recovered.claim_message("steering", &[]).await.is_some(),
                "uncheckpointed claim is retryable after restart"
            );
        }
        let consumed = services(
            directory.path(),
            vec![json!({"id":"old","kind":"steering","status":"pending"})],
        );
        assert!(
            consumed
                .claim_message("steering", &["old".into()])
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancellation_reservation_blocks_claim_and_rollback_restores_delivery() {
        let directory = tempfile::tempdir().unwrap();
        let queue = vec![json!({"id":"message","kind":"followup","status":"pending"})];
        let service = services(directory.path(), queue.clone());
        let reservation = service
            .reserve_message_cancellation("message")
            .await
            .unwrap();
        let mut claim = Box::pin(service.claim_message("followup", &[]));
        assert!(futures::poll!(claim.as_mut()).is_pending());
        // A rejected durable write drops the reservation without changing state.
        service.replace_queue(queue.clone());
        drop(reservation);
        assert_eq!(claim.await.unwrap()["id"], "message");
        assert!(
            service
                .reserve_message_cancellation("message")
                .await
                .is_err()
        );

        let service = services(directory.path(), queue);
        let reservation = service
            .reserve_message_cancellation("message")
            .await
            .unwrap();
        let mut claim = Box::pin(service.claim_message("followup", &[]));
        assert!(futures::poll!(claim.as_mut()).is_pending());
        reservation.commit();
        assert!(claim.await.is_none());
    }

    #[tokio::test]
    async fn queued_skill_keeps_submission_content_and_hash_after_edit_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".pi/skills/demo/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "---\nname: demo\ndescription: Test skill\n---\nBody at submission\n";
        std::fs::write(&path, original).unwrap();
        let context = ContextSnapshot::load_with_home(
            directory.path(),
            &[],
            Some(&directory.path().join("fixture-home")),
        )
        .await
        .unwrap();
        let (text, metadata) = context
            .expand_skill_with_metadata("/skill:demo queued arguments")
            .unwrap();
        let metadata = metadata.unwrap();
        let expected_hash = format!("{:x}", Sha256::digest(original));
        assert_eq!(metadata["hash"], expected_hash);
        assert_eq!(metadata["path"], json!(path));
        assert!(text.contains(&format!("hash=\"{expected_hash}\"")));
        let queue = vec![
            json!({"id":"queued-skill","kind":"followup","text":text,"originalText":"/skill:demo queued arguments","status":"pending"}),
        ];
        // The same serialized envelope is persisted by the message API. File
        // lookup must not happen again when a graph claims it after recovery.
        let persisted = serde_json::to_vec(&queue).unwrap();
        std::fs::write(
            &path,
            "---\nname: demo\ndescription: Changed skill\n---\nLater body\n",
        )
        .unwrap();
        let restored_queue: Vec<Value> = serde_json::from_slice(&persisted).unwrap();
        let recovered = services(directory.path(), restored_queue);
        let claimed = recovered.claim_message("followup", &[]).await.unwrap();
        assert_eq!(claimed["text"], text);
        assert!(
            claimed["text"]
                .as_str()
                .unwrap()
                .contains("Body at submission")
        );
        assert!(!claimed["text"].as_str().unwrap().contains("Later body"));
        assert!(
            claimed["text"]
                .as_str()
                .unwrap()
                .ends_with("queued arguments")
        );
        assert!(recovered.claim_message("followup", &[]).await.is_none());
        let (_, newer) = context.expand_skill_with_metadata("/skill:demo").unwrap();
        assert_ne!(newer.unwrap()["hash"], metadata["hash"]);
        std::fs::remove_file(path).unwrap();
        let recovered = services(
            directory.path(),
            serde_json::from_slice(&persisted).unwrap(),
        );
        assert_eq!(
            recovered.claim_message("followup", &[]).await.unwrap()["text"],
            text
        );
    }

    #[tokio::test]
    async fn skill_read_event_identifies_content_actually_returned() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".pi/skills/demo/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "---\nname: demo\ndescription: Test skill\n---\nOnly this body\n",
        )
        .unwrap();
        let context = ContextSnapshot::load_with_home(
            directory.path(),
            &[],
            Some(&directory.path().join("fixture-home")),
        )
        .await
        .unwrap();
        let service = RunServices::new(
            "skill-read".into(),
            directory.path().to_path_buf(),
            directory.path().join("data"),
            context,
            json!({}),
            vec![],
        )
        .unwrap();
        let (sender, mut events) = crate::event_sink::channel(64);
        service.set_sender(Some(sender));
        let result = service
            .execute_tool("tools", "read-skill", "read", json!({"path":path}))
            .await
            .unwrap();
        let mut found = None;
        while let Ok(event) = events.try_recv() {
            if event["type"] == "skill_loaded" {
                found = Some(event);
            }
        }
        let event = found.expect("skill load is observable");
        assert_eq!(event["name"], "demo");
        assert_eq!(
            event["hash"],
            format!("{:x}", Sha256::digest(result["content"].as_str().unwrap()))
        );
    }
}
