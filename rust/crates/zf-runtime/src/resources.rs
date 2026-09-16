//! Native acquisition adapters bound explicitly to a flow directory.
//! Pure reader contracts and validation stay in zf-context; this host owns
//! cancellation, immutable content references and shared resident values.
use anyhow::{Context, Result, ensure};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use sqlx::Connection;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak},
};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use zf_context::resource_readers::{
    ReadResource, ReaderContract, ReaderRegistry, ResourceReader, standard_contracts,
};
use zf_core::types::{DataType, TypeRegistry};
use zf_storage::content_store::ContentStore;

const MAX_BYTES: usize = 16 * 1024 * 1024;

/// A run's acquisition services. Clones share the resident cache; each flow
/// explicitly creates its registry with its own working directory.
#[derive(Clone)]
pub struct ResourceReads {
    content: Option<ContentStore>,
    cancel: CancellationToken,
    values: Arc<Mutex<BTreeMap<String, Weak<Value>>>>,
}
impl ResourceReads {
    pub fn new(content: Option<ContentStore>, cancel: CancellationToken) -> Self {
        Self {
            content,
            cancel,
            values: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Installs native implementations only on explicit host request. Reading
    /// the authoring catalogue alone never opens resources or installs readers.
    pub fn native_readers(&self, cwd: PathBuf) -> Result<ReaderRegistry> {
        let mut registry = ReaderRegistry::new();
        for contract in standard_contracts() {
            let kind = match contract.id.as_str() {
                "file.text" => Builtin::FileText,
                "file.json" => Builtin::FileJson,
                "sqlite.json" => Builtin::SqliteJson,
                "content.text" => Builtin::ContentText,
                "content.json" => Builtin::ContentJson,
                id => anyhow::bail!("Native resource reader is not implemented: {id}"),
            };
            registry.register(Arc::new(NativeReader {
                kind,
                contract,
                cwd: cwd.clone(),
                content: self.content.clone(),
            }))?;
        }
        Ok(registry)
    }

    /// Validates via the pure registry before persisting anything. Extensions
    /// use this same boundary, so cancellation and provenance are not optional.
    pub async fn read(
        &self,
        registry: &ReaderRegistry,
        id: &str,
        input: &Value,
        expected: &DataType,
        types: &TypeRegistry,
    ) -> Result<Option<ReadResource>> {
        let result = tokio::select! { biased;
            _ = self.cancel.cancelled() => anyhow::bail!("Resource read cancelled"),
            result = registry.read(id, input, expected, types) => result?,
        };
        let Some(mut result) = result else {
            return Ok(None);
        };
        let (output_ref, input_ref) = if let Some(store) = &self.content {
            (
                Some(store.intern(&result.value).await?),
                Some(store.intern(input).await?),
            )
        } else {
            (None, None)
        };
        if let Some(reference) = &output_ref {
            let mut cache = self.values.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(value) = cache.get(reference).and_then(Weak::upgrade) {
                result.value = value;
            } else {
                if cache.len() >= 256 {
                    cache.retain(|_, value| value.strong_count() > 0);
                    if cache.len() >= 256 {
                        cache.pop_first();
                    }
                }
                cache.insert(reference.clone(), Arc::downgrade(&result.value));
            }
        }
        // The registry already wrapped native provenance; replace that one
        // envelope instead of nesting an extra reader use around it.
        let contract = result.provenance["reader"].take();
        let source = result.provenance["source"].take();
        result.provenance = json!({"kind":"reader","reader":contract,"inputRef":input_ref,
            "input":if input_ref.is_none(){Some(input)}else{None},"contentRef":output_ref,"source":source});
        Ok(Some(result))
    }
}

#[derive(Clone, Copy)]
enum Builtin {
    FileText,
    FileJson,
    SqliteJson,
    ContentText,
    ContentJson,
}
struct NativeReader {
    kind: Builtin,
    contract: ReaderContract,
    cwd: PathBuf,
    content: Option<ContentStore>,
}
impl ResourceReader for NativeReader {
    fn contract(&self) -> ReaderContract {
        self.contract.clone()
    }
    fn read<'a>(&'a self, input: &'a Value) -> BoxFuture<'a, Result<Option<ReadResource>>> {
        Box::pin(async move {
            let (value, provenance) = match self.kind {
                Builtin::FileText | Builtin::FileJson => {
                    let path = path(
                        &self.cwd,
                        input["path"].as_str().context("Reader path absent")?,
                    );
                    let Some(bytes) = file_bytes(&path).await? else {
                        return Ok(None);
                    };
                    let hash = byte_hash(&bytes);
                    let value = if matches!(self.kind, Builtin::FileText) {
                        Value::String(
                            String::from_utf8(bytes).context("Reader source is not UTF-8")?,
                        )
                    } else {
                        serde_json::from_slice(&bytes).context("Reader source is not JSON")?
                    };
                    (value, json!({"path":path,"hash":hash}))
                }
                Builtin::SqliteJson => {
                    let path = path(
                        &self.cwd,
                        input["path"].as_str().context("SQLite path absent")?,
                    );
                    match tokio::fs::metadata(&path).await {
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                        Err(e) => return Err(e.into()),
                        Ok(m) => ensure!(
                            m.is_file(),
                            "SQLite reader requires a regular database file"
                        ),
                    }
                    let table = input["table"].as_str().context("SQLite table absent")?;
                    ensure!(
                        !table.is_empty()
                            && table.len() <= 128
                            && table
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
                        "SQLite table must be an identifier"
                    );
                    let id = input["id"].as_str().context("SQLite row identity absent")?;
                    let mut connection = sqlx::SqliteConnection::connect_with(
                        &sqlx::sqlite::SqliteConnectOptions::new()
                            .filename(&path)
                            .read_only(true)
                            .create_if_missing(false),
                    )
                    .await?;
                    let query = format!(
                        "SELECT CASE WHEN length(CAST(document AS BLOB)) <= {MAX_BYTES} THEN document END FROM \"{table}\" WHERE id=? LIMIT 2"
                    );
                    let mut rows: Vec<Option<String>> = sqlx::query_scalar(&query)
                        .bind(id)
                        .fetch_all(&mut connection)
                        .await?;
                    connection.close().await?;
                    ensure!(rows.len() <= 1, "SQLite row identity is ambiguous");
                    let Some(text) = rows.pop() else {
                        return Ok(None);
                    };
                    let text = text.context("SQLite document is null or exceeds reader limit")?;
                    let value =
                        serde_json::from_str(&text).context("SQLite document is not JSON")?;
                    (
                        value,
                        json!({"path":path,"table":table,"id":id,"hash":byte_hash(text.as_bytes())}),
                    )
                }
                Builtin::ContentText | Builtin::ContentJson => {
                    let reference = input["contentRef"]
                        .as_str()
                        .context("Content reference absent")?;
                    let store = self.content.as_ref().context("Content store unavailable")?;
                    let max = if matches!(self.kind, Builtin::ContentJson) {
                        MAX_BYTES
                    } else {
                        MAX_BYTES * 2
                    };
                    let stored = store.resolve_with_limit(reference, max as u64).await?;
                    let value = if matches!(self.kind, Builtin::ContentJson) || stored.is_string() {
                        stored
                    } else {
                        Value::String(
                            String::from_utf8(zf_storage::content_store::decode_full_output(
                                &stored,
                            )?)
                            .context("Content is not UTF-8")?,
                        )
                    };
                    if let Value::String(text) = &value {
                        ensure!(text.len() <= MAX_BYTES, "Content text exceeds 16 MiB");
                    }
                    (value, json!({"contentRef":reference}))
                }
            };
            Ok(Some(ReadResource {
                value: Arc::new(value),
                provenance,
            }))
        })
    }
}
fn path(cwd: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}
fn byte_hash(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}
async fn file_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(metadata.is_file(), "Reader source must be a regular file");
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    // Refuse special files before opening and avoid a blocking FIFO open if an
    // external writer swaps the path between metadata and open.
    #[cfg(unix)]
    options.custom_flags(nix::libc::O_NONBLOCK);
    let file = match options.open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let metadata = file.metadata().await?;
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_BYTES as u64,
        "Reader source must be a regular file of at most 16 MiB"
    );
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    ensure!(bytes.len() <= MAX_BYTES, "Reader source exceeds 16 MiB");
    Ok(Some(bytes))
}

/// Captured state inputs are intentionally left to that invocation. This helper
/// performs metadata checks and never opens a database or reads a source body.
pub fn dependency_diagnostics(
    doc: &zf_flows::schema::Composition,
    cwd: &Path,
) -> Vec<zf_core::diagnostics::Diagnostic> {
    fn visit(
        doc: &zf_flows::schema::Composition,
        cwd: &Path,
        prefix: &str,
        out: &mut Vec<zf_core::diagnostics::Diagnostic>,
    ) {
        for node in &doc.nodes {
            let node_path = if prefix.is_empty() {
                node.id.clone()
            } else {
                format!("{prefix}/{}", node.id)
            };
            if node.data.kind == "subgraph" {
                if let Ok(child) = serde_json::from_value(node.data.config["composition"].clone()) {
                    visit(&child, cwd, &node_path, out);
                }
                continue;
            }
            let bindings = node.data.config["contextProgram"]
                .get("bindings")
                .unwrap_or(&node.data.config["contextBindings"]);
            for (name, binding) in bindings.as_object().into_iter().flatten() {
                if binding["kind"] != "reader"
                    || binding["input"]["kind"] != "literal"
                    || !matches!(
                        binding["reader"].as_str(),
                        Some("file.text" | "file.json" | "sqlite.json")
                    )
                {
                    continue;
                }
                let Some(source) = binding["input"]["value"]["path"].as_str() else {
                    continue;
                };
                let source = path(cwd, source);
                let reason = match std::fs::metadata(&source) {
                    Ok(metadata) if metadata.is_file() => continue,
                    Ok(_) => "source is not a regular file".into(),
                    Err(error) => error.to_string(),
                };
                out.push(zf_core::diagnostics::Diagnostic::new("reader_dependency",format!("{node_path}/bindings/{name}"),format!("{}: {reason}; required only if this resource is selected at the next invocation",source.display())));
            }
        }
    }
    let mut diagnostics = Vec::new();
    visit(doc, cwd, "", &mut diagnostics);
    diagnostics
}

pub mod producers {
    //! Demand-driven durable producer invocation through the injected route host.
    use crate::runtime::{BranchInvocation, RouteOutcome, RunServices};
    use anyhow::{Context, Result, ensure};
    use serde_json::{Value, json};
    use std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        sync::Arc,
    };
    use zf_context::{
        context::{self, ContextEvaluation, ContextItem},
        context_resources::{PreparedResources, input, selected_output_type},
        resources::{ContextProgram, ResourceBinding},
        window::agent_scope,
    };
    use zf_core::{
        identity::Revision,
        types::{DataType, compatible, validate_value},
    };
    use zf_storage::data::DataError;

    fn digest(value: &Value) -> Result<String> {
        {
            use sha2::Digest;
            Ok(format!(
                "{:x}",
                sha2::Sha256::digest(serde_json::to_vec(value)?)
            ))
        }
    }

    struct Request {
        key: String,
        alias: String,
        input: Value,
        descriptor: Value,
        sources: Value,
    }
    async fn request(
        program: &ContextProgram,
        services: &RunServices,
        path: &str,
        name: &str,
        evaluated: &ContextEvaluation,
        provenance: &BTreeMap<String, Value>,
        state: &HashMap<String, Value>,
    ) -> Result<Request> {
        let Some(ResourceBinding::Produced { producer }) = program.bindings.get(name) else {
            anyhow::bail!("Resource has no producer binding: {name}");
        };
        let host = services
            .dynamic_capabilities()
            .context("A producer requires a composed route runtime")?;
        let descriptor = host
            .route_contract(path, &producer.branch, Some(&producer.route_id))
            .await?;
        ensure!(
            descriptor["mode"] == "callAwait" && descriptor["invocation"] == "context",
            "A resource producer must use an explicit call-await context route"
        );
        if let Some(condition) = descriptor.get("condition").filter(|value| !value.is_null()) {
            ensure!(
                crate::predicates::evaluate(
                    &zf_flows::node_contracts::parse_predicate(condition)?,
                    state
                )?,
                "The producer route is not eligible in this node state"
            );
        }
        let expected: DataType = serde_json::from_value(descriptor["input"].clone())?;
        let actual = context::expression_type(
            &producer.input,
            &program.strategy.requirements,
            &program.types,
            &program.library,
        )
        .map_err(|d| anyhow::anyhow!("Producer input: {d:?}"))?;
        ensure!(
            compatible(&actual, &expected, &program.types),
            "Producer input type differs from the composed route contract"
        );
        let Some(ContextItem::Fragment { value, .. }) = evaluated.items.first() else {
            anyhow::bail!("Producer input is incomplete");
        };
        let input = serde_json::to_value(value)?;
        validate_value(&expected, &input, &program.types)
            .map_err(|d| anyhow::anyhow!("Producer input: {d:?}"))?;
        let sources: BTreeMap<_, _> = evaluated
            .reads
            .iter()
            .map(|name| {
                (
                    name.clone(),
                    provenance
                        .get(name)
                        .cloned()
                        .unwrap_or(json!({"absent":true})),
                )
            })
            .collect();
        let sources = serde_json::to_value(sources)?;
        let input_ref = services
            .content_store()
            .context("Producer content store unavailable")?
            .intern(&input)
            .await?;
        let key = digest(
            &json!({"path":path,"resource":name,"producer":producer,"descriptor":descriptor,"inputRef":input_ref,"sources":sources}),
        )?;
        let alias = format!("produced:{}", digest(&json!([path, name]))?);
        Ok(Request {
            key,
            alias,
            input,
            descriptor,
            sources,
        })
    }

    pub async fn prepare(
        program: &ContextProgram,
        services: &RunServices,
        path: &str,
        state: &HashMap<String, Value>,
        ctx: Option<&adk_graph::NodeContext>,
        resources: &mut BTreeMap<String, Arc<Value>>,
        provenance: &mut BTreeMap<String, Value>,
    ) -> Result<PreparedResources> {
        let mut statuses = BTreeMap::new();
        let mut failures = BTreeMap::new();
        // Only cache reads are performed here. Presence probes never launch a flow.
        for _ in 0..=program.bindings.len() {
            let mut changed = false;
            for (name, binding) in &program.bindings {
                let ResourceBinding::Produced { producer } = binding else {
                    continue;
                };
                if resources.contains_key(name) {
                    continue;
                }
                let evaluated = input(program, producer, resources);
                if !evaluated.complete {
                    continue;
                }
                let req =
                    match request(program, services, path, name, &evaluated, provenance, state)
                        .await
                    {
                        Ok(req) => req,
                        Err(error) => {
                            failures.insert(name.clone(), error.to_string());
                            continue;
                        }
                    };
                if let Some(saved) = services
                    .read_record("context-production-results", &req.key)
                    .await?
                {
                    let registry = services
                        .data_registry()
                        .context("Producer registry unavailable")?;
                    let revision: Revision = serde_json::from_value(saved["revision"].clone())?;
                    let snapshot = registry
                        .revision(&agent_scope(path), &req.alias, &revision)
                        .await?;
                    provenance.insert(name.clone(), json!({"kind":"produced","producerKey":req.key,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"entityId":snapshot.entity_id}));
                    resources.insert(name.clone(), snapshot.value);
                    statuses.insert(
                    name.clone(),
                    json!({"status":"ready","producerKey":req.key,"revision":snapshot.revision}),
                );
                    changed = true;
                } else {
                    let status = services
                        .read_record("context-production-status", &req.key)
                        .await?;
                    let prior = if let Some(registry) = services.data_registry() {
                        registry.snapshot(&agent_scope(path), &req.alias).await.ok()
                    } else {
                        None
                    };
                    statuses.insert(name.clone(), status.unwrap_or_else(|| json!({"status":if prior.is_some(){"stale"}else{"missing"},"producerKey":req.key})));
                }
            }
            if !changed {
                break;
            }
        }
        let mut attempted = BTreeSet::new();
        let mut input_reads = BTreeSet::new();
        let mut wait = None;
        let mut evaluation = context::evaluate_with_library(
            &program.strategy,
            resources,
            &program.types,
            &program.library,
        );
        // Each producer is dispatched at most once per preparation. Dependency
        // demands use the same pure evaluator and are bounded by declared bindings.
        for _ in 0..=program.bindings.len() {
            let mut pending = evaluation.needs.clone();
            let mut seen = BTreeSet::new();
            let mut ready = None;
            while let Some(need) = pending.pop() {
                if !seen.insert(need.resource.clone()) {
                    continue;
                }
                let Some(ResourceBinding::Produced { producer }) =
                    program.bindings.get(&need.resource)
                else {
                    continue;
                };
                if attempted.contains(&need.resource) {
                    continue;
                }
                let evaluated = input(program, producer, resources);
                input_reads.extend(evaluated.reads.iter().cloned());
                if !evaluated.diagnostics.is_empty() {
                    anyhow::bail!("Invalid producer input: {:?}", evaluated.diagnostics);
                }
                if evaluated.complete {
                    ready = Some((need.resource, producer, evaluated));
                    break;
                }
                for dependency in &evaluated.needs {
                    if !evaluation
                        .needs
                        .iter()
                        .any(|n| n.resource == dependency.resource)
                    {
                        evaluation.needs.push(dependency.clone());
                    }
                }
                pending.extend(evaluated.needs);
            }
            let Some((name, producer, evaluated)) = ready else {
                break;
            };
            let ctx = ctx.context("Executing a producer requires an actual ADK NodeContext")?;
            attempted.insert(name.clone());
            let req = request(
                program, services, path, &name, &evaluated, provenance, state,
            )
            .await?;
            let output_type = selected_output_type(producer, &req.descriptor, program)?;
            ensure!(
                compatible(
                    &output_type,
                    &program.strategy.requirements[&name],
                    &program.types
                ),
                "Producer output type differs from the resource requirement: {name}"
            );
            let registry = services
                .data_registry()
                .context("Producer registry unavailable")?;
            let scope = agent_scope(path);
            let expected = match registry.snapshot(&scope, &req.alias).await {
                Ok(snapshot) => Some(snapshot.revision),
                Err(DataError::NotFound) => None,
                Err(error) => return Err(error.into()),
            };
            let manifest = if let Some(saved) = services
                .read_record("context-productions", &req.key)
                .await?
            {
                saved
            } else {
                let manifest = json!({"version":1,"nodePath":path,"resource":name,"input":req.input,"descriptor":req.descriptor,"sources":req.sources,"alias":req.alias,"expectedRevision":expected});
                services
                    .persist_record("context-productions", &req.key, &manifest)
                    .await?;
                manifest
            };
            let store = services
                .content_store()
                .context("Producer content store unavailable")?;
            let running = json!({"status":"running","producerKey":req.key,"nodePath":path,"resource":name,"threadId":ctx.config.thread_id,"step":ctx.step});
            store
                .put_record(
                    &services.id,
                    "context-production-status",
                    &req.key,
                    &running,
                )
                .await?;
            services.emit(json!({"type":"context_resource_status","nodePath":path,"resource":name,"status":running})).await;
            let host = services
                .dynamic_capabilities()
                .context("Producer route runtime unavailable")?;
            let outcome = host
                .invoke_branch(BranchInvocation {
                    path: path.into(),
                    branch: producer.branch.clone(),
                    invocation: zf_flows::composition::InvocationKind::Context,
                    route_id: Some(producer.route_id.clone()),
                    call_id: format!("producer:{}", req.key),
                    input: req.input,
                    caller_state: state.clone(),
                })
                .await;
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => {
                    let failed =
                        json!({"status":"failed","producerKey":req.key,"error":error.to_string()});
                    store
                        .put_record(&services.id, "context-production-status", &req.key, &failed)
                        .await?;
                    statuses.insert(name.clone(), failed.clone());
                    services.emit(json!({"type":"context_resource_status","nodePath":path,"resource":name,"status":failed})).await;
                    return Err(error);
                }
            };
            let result = match outcome {
                RouteOutcome::Completed { result, .. } => result,
                RouteOutcome::Waiting {
                    visit_id,
                    thread_id,
                    wait: child,
                } => {
                    wait = Some(
                        json!({"kind":"context_production","nodePath":path,"resource":name,"producerKey":req.key,"visitId":visit_id,"threadId":thread_id,"data":child}),
                    );
                    statuses.insert(name, running);
                    break;
                }
                _ => anyhow::bail!(
                    "A producer must complete or durably wait in its call-await route"
                ),
            };
            let value = producer
                .output_pointer
                .as_ref()
                .map_or(Some(&result), |pointer| result.pointer(pointer))
                .context("Producer result did not contain its declared output")?;
            validate_value(&program.strategy.requirements[&name], value, &program.types)
                .map_err(|d| anyhow::anyhow!("Producer output: {d:?}"))?;
            let expected: Option<Revision> =
                serde_json::from_value(manifest["expectedRevision"].clone())?;
            let snapshot = registry
                .publish_unique(
                    &scope,
                    &req.alias,
                    expected.as_ref(),
                    value,
                    &format!("producer:{}", req.key),
                )
                .await?;
            let completed = json!({"status":"ready","producerKey":req.key,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"entityId":snapshot.entity_id});
            services
                .persist_record("context-production-results", &req.key, &completed)
                .await?;
            store
                .put_record(
                    &services.id,
                    "context-production-status",
                    &req.key,
                    &completed,
                )
                .await?;
            services.emit(json!({"type":"context_resource_status","nodePath":path,"resource":name,"status":completed})).await;
            statuses.insert(name.clone(), completed.clone());
            provenance.insert(name.clone(), json!({"kind":"produced","producerKey":req.key,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"entityId":snapshot.entity_id}));
            resources.insert(name, snapshot.value);
            evaluation = context::evaluate_with_library(
                &program.strategy,
                resources,
                &program.types,
                &program.library,
            );
        }
        for name in &evaluation.reads {
            if let Some(error) = failures.get(name) {
                evaluation.diagnostics.push(zf_core::types::Diagnostic::new(
                    "producer_contract",
                    format!("bindings.{name}"),
                    error,
                ));
                evaluation.complete = false;
            }
        }
        evaluation.reads.extend(input_reads);
        evaluation.reads.sort();
        evaluation.reads.dedup();
        if wait.is_none() && !evaluation.needs.is_empty() {
            let mut edges = BTreeMap::new();
            for need in &evaluation.needs {
                if let Some(ResourceBinding::Produced { producer }) =
                    program.bindings.get(&need.resource)
                {
                    let inputs = input(program, producer, resources);
                    edges.insert(
                        need.resource.clone(),
                        inputs
                            .needs
                            .into_iter()
                            .map(|n| n.resource)
                            .collect::<Vec<_>>(),
                    );
                }
            }
            fn cycle(
                name: &str,
                edges: &BTreeMap<String, Vec<String>>,
                active: &mut BTreeSet<String>,
                done: &mut BTreeSet<String>,
            ) -> bool {
                if done.contains(name) {
                    return false;
                }
                if !active.insert(name.into()) {
                    return true;
                }
                if edges.get(name).is_some_and(|children| {
                    children
                        .iter()
                        .any(|child| cycle(child, edges, active, done))
                }) {
                    return true;
                }
                active.remove(name);
                done.insert(name.into());
                false
            }
            let mut done = BTreeSet::new();
            if edges
                .keys()
                .any(|name| cycle(name, &edges, &mut BTreeSet::new(), &mut done))
            {
                evaluation.diagnostics.push(zf_core::types::Diagnostic::new(
                    "producer_cycle",
                    "bindings",
                    "The active producer input dependencies contain a cycle",
                ));
                evaluation.complete = false;
            }
        }
        Ok(PreparedResources {
            evaluation,
            statuses,
            wait,
        })
    }
}

pub mod window_preparation {
    //! Durable selection commands and preparation tied to actual ADK passages.
    use crate::runtime::{BranchInvocation, RouteOutcome, RunServices};
    use anyhow::{Context, Result, ensure};
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, sync::Arc};
    use zf_context::{
        context::ContextEvaluation,
        resources::ContextProgram,
        window,
        window_preparation::{
            PreparedInvocationWindow, WindowPreparation, WindowSelectionCommand, items,
        },
    };
    use zf_core::identity::Revision;
    use zf_storage::data::{DataError, Snapshot, WindowRegistry};

    pub struct PreparationInputs<'a> {
        pub ctx: &'a adk_graph::NodeContext,
        pub evaluation: &'a ContextEvaluation,
        pub resources: &'a BTreeMap<String, Arc<Value>>,
        pub provenance: &'a BTreeMap<String, Value>,
    }

    fn hash(value: &Value) -> Result<String> {
        {
            use sha2::Digest;
            Ok(format!(
                "{:x}",
                sha2::Sha256::digest(serde_json::to_vec(value)?)
            ))
        }
    }
    fn owner_key(path: &str, alias: &str) -> Result<String> {
        hash(&json!([window::agent_scope(path), alias]))
    }

    /// A UI command selects an existing revision; it neither invents a future node
    /// state nor executes a preparation flow. Only one unconsumed choice per agent
    /// is accepted, and re-sending the same command identity is idempotent.
    pub async fn queue_selection(
        services: &RunServices,
        command: &WindowSelectionCommand,
    ) -> Result<Value> {
        uuid::Uuid::parse_str(&command.id)?;
        let owner = services
            .read_record(
                "window-owners",
                &owner_key(&command.node_path, &command.alias)?,
            )
            .await?
            .context("Window has not been prepared by this agent")?;
        ensure!(
            owner["nodePath"] == command.node_path,
            "Window belongs to another agent"
        );
        let registry = WindowRegistry::new(
            services
                .data_registry()
                .context("Window registry unavailable")?,
        );
        let snapshot = registry
            .revision(
                &window::agent_scope(&command.node_path),
                &command.alias,
                &command.revision,
            )
            .await?;
        let window = window::decode(snapshot.value.as_ref())?;
        ensure!(
            window
                .program_revision
                .as_ref()
                .unwrap_or(&window.strategy_revision)
                == &command.program_hash,
            "Selected window belongs to another program revision"
        );
        let store = services
            .content_store()
            .context("Window command storage unavailable")?;
        zf_storage::data::queue_window_selection(&store, &services.id, command).await
    }

    async fn claim_selection(
        services: &RunServices,
        path: &str,
        occurrence: &str,
    ) -> Result<Option<WindowSelectionCommand>> {
        let store = services
            .content_store()
            .context("Window command storage unavailable")?;
        zf_storage::data::claim_window_selection(&store, &services.id, path, occurrence).await
    }

    fn captured(
        snapshot: &Snapshot,
        alias: &str,
        origin: Value,
        selection: Option<&WindowSelectionCommand>,
    ) -> Value {
        json!({"alias":alias,"entityId":snapshot.entity_id,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"origin":origin,"selectionCommandId":selection.map(|c|&c.id)})
    }

    pub async fn prepare(
        settings: &WindowPreparation,
        program: &ContextProgram,
        services: &RunServices,
        path: &str,
        inputs: PreparationInputs<'_>,
    ) -> Result<PreparedInvocationWindow> {
        let PreparationInputs {
            ctx,
            evaluation,
            resources,
            provenance,
        } = inputs;
        let store = services
            .content_store()
            .context("Window content store unavailable")?;
        let data = services
            .data_registry()
            .context("Window registry unavailable")?;
        let registry = WindowRegistry::new(data.clone());
        let scope = window::agent_scope(path);
        let origin = json!({"nodePath":path,"threadId":ctx.config.thread_id,"step":ctx.step});
        let program_revision = program.revision()?;
        let occurrence = hash(&json!({"origin":origin,"programHash":program_revision}))?;
        let owner = json!({"nodePath":path,"alias":settings.alias});
        services
            .persist_record("window-owners", &owner_key(path, &settings.alias)?, &owner)
            .await?;
        if let Some(manifest) = services
            .read_record("window-invocation-results", &occurrence)
            .await?
        {
            let revision: Revision = serde_json::from_value(manifest["revision"].clone())?;
            let snapshot = registry
                .revision(&scope, &settings.alias, &revision)
                .await?;
            return Ok(PreparedInvocationWindow {
                items: Some(items(window::decode(snapshot.value.as_ref())?.items)),
                manifest,
                wait: None,
            });
        }
        let selection = claim_selection(services, path, &occurrence).await?;
        let selected = if let Some(command) = &selection {
            ensure!(
                command.alias == settings.alias && command.program_hash == program_revision,
                "Queued window selection no longer matches the agent program"
            );
            registry
                .revision(&scope, &settings.alias, &command.revision)
                .await?
        } else {
            let initial = if let Some(initial) =
                services.read_record("window-captures", &occurrence).await?
            {
                initial
            } else {
                let mut revisions = BTreeMap::new();
                for name in &evaluation.reads {
                    let revision = if let Some(value) = resources.get(name) {
                        if let Some(revision) =
                            provenance.get(name).and_then(|p| p["revision"].as_str())
                        {
                            revision.into()
                        } else {
                            store.intern(value).await?
                        }
                    } else {
                        format!("absent:{}", hash(&json!(name))?)
                    };
                    revisions.insert(name.clone(), revision);
                }
                let mut window =
                    window::capture(&program.strategy, &program.hash, evaluation, revisions)?;
                window.program_revision = Some(program_revision.clone());
                let expected = match data.snapshot(&scope, &settings.alias).await {
                    Ok(snapshot) => {
                        window::decode(snapshot.value.as_ref())?;
                        Some(snapshot.revision)
                    }
                    Err(DataError::NotFound) => None,
                    Err(error) => return Err(error.into()),
                };
                let initial = json!({"origin":origin,"programHash":program_revision,"expectedRevision":expected,"window":window,"stateRef":store.intern(&serde_json::to_value(&ctx.state)?).await?});
                services
                    .persist_record("window-captures", &occurrence, &initial)
                    .await?;
                initial
            };
            let expected: Option<Revision> =
                serde_json::from_value(initial["expectedRevision"].clone())?;
            let initial_snapshot = data
                .publish_unique(
                    &scope,
                    &settings.alias,
                    expected.as_ref(),
                    &initial["window"],
                    &format!("window-capture:{occurrence}"),
                )
                .await?;
            if let Some(route) = &settings.prepare {
                let host = services
                    .dynamic_capabilities()
                    .context("Window preparation route unavailable")?;
                let contract = host
                    .route_contract(path, &route.branch, Some(&route.route_id))
                    .await?;
                ensure!(
                    contract["mode"] == "callAwait" && contract["invocation"] == "context",
                    "Window preparation requires a call-await context route"
                );
                let input = json!({"alias":settings.alias,"entityId":initial_snapshot.entity_id,"revision":initial_snapshot.revision,"contentRef":initial_snapshot.content_ref});
                let outcome = host
                    .invoke_branch(BranchInvocation {
                        path: path.into(),
                        branch: route.branch.clone(),
                        invocation: zf_flows::composition::InvocationKind::Context,
                        route_id: Some(route.route_id.clone()),
                        call_id: format!("window-prepare:{occurrence}"),
                        input,
                        caller_state: ctx.state.clone(),
                    })
                    .await?;
                match outcome {
                    RouteOutcome::Completed { .. } => {
                        registry.read(&scope, &settings.alias).await?
                    }
                    RouteOutcome::Waiting {
                        visit_id,
                        thread_id,
                        wait,
                    } => {
                        return Ok(PreparedInvocationWindow {
                            items: None,
                            manifest: captured(&initial_snapshot, &settings.alias, origin, None),
                            wait: Some(
                                json!({"kind":"context_window_preparation","nodePath":path,"visitId":visit_id,"threadId":thread_id,"data":wait}),
                            ),
                        });
                    }
                    _ => anyhow::bail!(
                        "Window preparation must complete or wait in its call-await route"
                    ),
                }
            } else {
                initial_snapshot
            }
        };
        let window = window::decode(selected.value.as_ref())?;
        ensure!(
            window.program_revision.as_deref() == Some(&program_revision)
                && window.capabilities == evaluation.capabilities,
            "Selected window cannot replace the program or capability grants"
        );
        let manifest = captured(&selected, &settings.alias, origin, selection.as_ref());
        services
            .persist_record("window-invocation-results", &occurrence, &manifest)
            .await?;
        services
            .emit(json!({"type":"context_window_prepared","nodePath":path,"window":manifest}))
            .await;
        Ok(PreparedInvocationWindow {
            items: Some(items(window.items)),
            manifest,
            wait: None,
        })
    }
}

pub mod window {
    //! Model-sealed window operations with captured grants and durable publications.
    use serde::Deserialize;
    use serde_json::Value;
    use zf_context::window::{WindowPatch, agent_scope, grants, is_tool};
    use zf_core::identity::{Permission, Revision};
    use zf_storage::data::WindowRegistry;
    pub async fn execute_tool(
        services: &crate::runtime::RunServices,
        path: &str,
        call_id: &str,
        name: &str,
        arguments: &Value,
    ) -> anyhow::Result<Value> {
        use anyhow::{Context, ensure};
        use serde_json::json;
        ensure!(is_tool(name), "Unknown context window capability");
        let (invocation, index) = call_id
            .rsplit_once(':')
            .context("Window operation requires a sealed model call")?;
        uuid::Uuid::parse_str(invocation)?;
        let index: usize = index.parse()?;
        let record = services
            .read_record("model-calls", invocation)
            .await?
            .context("Unknown model invocation")?;
        let call = record["calls"].get(index).context("Unknown sealed call")?;
        ensure!(
            record["agentPath"] == path && call["name"] == name && call["args"] == *arguments,
            "Window call differs from its durable model provenance"
        );
        ensure!(
            record["tools"]
                .as_array()
                .is_some_and(|tools| tools.contains(&json!(name))),
            "Window capability was not selected"
        );
        let snapshot = services
            .read_record("capability-snapshots", invocation)
            .await?
            .context("Captured window grants unavailable")?;
        ensure!(
            snapshot["agentPath"] == path,
            "Window grants belong to another agent"
        );
        let captured = json!({"windowGrants":snapshot["prepared"]["windowGrants"]});
        let alias = arguments["alias"]
            .as_str()
            .context("Window alias missing")?;
        ensure!(
            grants(&captured)?.iter().any(|g| g.alias == alias
                && (name == "context_window_read" || g.permission == Permission::Write)),
            "Window alias permission was not granted to this agent"
        );
        let scope = agent_scope(path);
        let registry = WindowRegistry::new(
            services
                .data_registry()
                .context("Window registry unavailable")?,
        );
        let snapshot = if name == "context_window_read" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Read {
                alias: String,
                revision: Option<Revision>,
            }
            let request: Read = serde_json::from_value(arguments.clone())?;
            if let Some(revision) = request.revision {
                registry.revision(&scope, &request.alias, &revision).await?
            } else {
                registry.read(&scope, &request.alias).await?
            }
        } else {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Patch {
                alias: String,
                expected_revision: Revision,
                patches: Vec<WindowPatch>,
            }
            let request: Patch = serde_json::from_value(arguments.clone())?;
            registry
                .patch_unique(
                    &scope,
                    &request.alias,
                    &request.expected_revision,
                    &request.patches,
                    &format!("window:{path}:{call_id}"),
                )
                .await?
        };
        Ok(
            json!({"entityId":snapshot.entity_id,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"window":snapshot.value}),
        )
    }
}

pub mod request_preview {
    //! Explicit trial profiles and provider request serialization without acquisition.
    use crate::inference::{self, MediaSource};
    use adk_core::{GenerateContentConfig, LlmRequest};
    use anyhow::{Result, ensure};
    use serde::Deserialize;
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, sync::Arc};
    use zf_context::{
        context::ContextEvaluation,
        request_preview::{self, PreviewTarget},
        resources::ContextProgram,
    };
    use zf_core::types::Diagnostic;
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct PreviewProfile {
        pub provider: String,
        pub model: String,
        #[serde(deserialize_with = "configuration")]
        pub config: GenerateContentConfig,
        #[serde(default)]
        pub tools: BTreeMap<String, Value>,
        #[serde(default)]
        pub media: BTreeMap<String, Value>,
        #[serde(default)]
        pub reasoning_effort: Option<String>,
        #[serde(default)]
        pub reasoning_summary: Option<String>,
        #[serde(default)]
        pub text_verbosity: Option<String>,
    }

    fn configuration<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<GenerateContentConfig, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("config doit être un objet"))?;
        for key in object.keys() {
            if !matches!(
                key.as_str(),
                "temperature"
                    | "top_p"
                    | "top_k"
                    | "frequency_penalty"
                    | "presence_penalty"
                    | "max_output_tokens"
                    | "seed"
                    | "top_logprobs"
                    | "stop_sequences"
                    | "response_schema"
                    | "cached_content"
                    | "extensions"
            ) {
                return Err(serde::de::Error::custom(format!(
                    "Paramètre de génération inconnu : {key}"
                )));
            }
        }
        serde_json::from_value(value).map_err(serde::de::Error::custom)
    }

    pub async fn prepare(
        profile: &PreviewProfile,
        program: &ContextProgram,
        evaluation: &ContextEvaluation,
        resources: &BTreeMap<String, Arc<Value>>,
    ) -> Value {
        match prepare_inner(profile, program, evaluation, resources).await {
            Ok(value) => value,
            Err(error) => json!({"status":"trial","diagnostics":[Diagnostic::new(
            "request_preview", "profile", format!("{error:#}"))]}),
        }
    }

    async fn prepare_inner(
        profile: &PreviewProfile,
        program: &ContextProgram,
        evaluation: &ContextEvaluation,
        resources: &BTreeMap<String, Arc<Value>>,
    ) -> Result<Value> {
        request_preview::validate(
            &PreviewTarget {
                provider: &profile.provider,
                model: &profile.model,
                tools: &profile.tools,
            },
            program,
            evaluation,
        )?;
        let tools = evaluation
            .capabilities
            .iter()
            .map(|capability| (capability.id.clone(), profile.tools[&capability.id].clone()))
            .collect();
        let contents = inference::adapt(
            &evaluation.items,
            program,
            resources,
            MediaSource::Trial(&profile.media),
            &profile.provider,
        )
        .await?;
        let request = LlmRequest {
            model: profile.model.clone(),
            contents,
            config: Some(profile.config.clone()),
            tools,
            previous_response_id: None,
        };
        let (boundary, body) = if profile.provider == "codex" {
            let options = json!({"__zedflowVersion":4,"reasoningEffort":profile.reasoning_effort,
            "reasoningSummary":profile.reasoning_summary,"textVerbosity":profile.text_verbosity});
            (
                "codexHttpBody",
                crate::codex::request_body(&profile.model, &options, &request)?,
            )
        } else {
            ensure!(
                profile.reasoning_effort.is_none()
                    && profile.reasoning_summary.is_none()
                    && profile.text_verbosity.is_none(),
                "Les options Codex ne s’appliquent pas à cette frontière"
            );
            let mut body = serde_json::to_value(&request)?;
            body["tools"] = json!(request.tools);
            (
                if profile.provider == "fixture" {
                    "fixtureInput"
                } else {
                    "adkRequest"
                },
                body,
            )
        };
        let parts = crate::inference_raw::segments(&body)?;
        let capture = crate::inference_raw::document(boundary, &parts);
        Ok(
            json!({"status":"prepared","boundary":boundary,"raw":parts.concat(),
        "byteLength":capture["byteLength"],"sha256":capture["sha256"],"diagnostics":[]}),
        )
    }
}

#[cfg(test)]
mod context_runtime_tests {
    use crate::{
        agent_capabilities::{self, EffectiveContext},
        inference, models,
        resources::window_preparation,
        runtime::{BranchInvocation, DynamicCapabilities, RouteOutcome, RunServices},
        workspace_context::ContextSnapshot,
    };
    use adk_graph::prelude::*;
    use anyhow::Result;
    use serde_json::{Value, json};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use zf_context::{
        context::*,
        context_source,
        window::{self, WindowPatch},
        window_preparation::WindowSelectionCommand,
    };
    use zf_core::identity::{Permission, Revision, Scope};
    use zf_storage::{
        content_store::ContentStore,
        data::{DataRegistry, WindowRegistry},
    };

    struct Fixture {
        _root: tempfile::TempDir,
        services: Arc<RunServices>,
        store: ContentStore,
    }
    impl Fixture {
        async fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(root.path().join("data.sqlite"))
                        .create_if_missing(true)
                        .foreign_keys(true),
                )
                .await
                .unwrap();
            let store = ContentStore::new(pool.clone()).await.unwrap();
            let services = RunServices::new(
                "preparation".into(),
                root.path().into(),
                root.path().join("data"),
                ContextSnapshot {
                    cwd: root.path().into(),
                    ..Default::default()
                },
                json!({}),
                vec![],
            )
            .unwrap();
            services.set_content_store(store.clone());
            services
                .set_data_registry(
                    DataRegistry::new(pool, store.clone(), &services.id)
                        .await
                        .unwrap(),
                )
                .unwrap();
            Self {
                _root: root,
                services,
                store,
            }
        }
        fn ctx(&self, step: usize, text: &str) -> NodeContext {
            NodeContext::new(
                State::from([("input".into(), json!(text))]),
                ExecutionConfig::new("preparation/root"),
                step,
            )
        }
        async fn run(&self, cfg: &Value, ctx: &NodeContext) -> NodeOutput {
            models::node_with_services("agent", cfg, "root/agent", self.services.clone())
                .unwrap()
                .execute(ctx)
                .await
                .unwrap()
        }
        async fn snapshot(&self, output: &NodeOutput) -> Value {
            self.services
                .read_record(
                    "capability-snapshots",
                    output.updates["modelResponse"]["contextSnapshotId"]
                        .as_str()
                        .unwrap(),
                )
                .await
                .unwrap()
                .unwrap()
        }
    }
    fn text_strategy() -> ContextStrategy {
        ContextStrategy::new("prepare", "Prepare")
            .require("input", zf_core::types::DataType::Text)
            .with_program(vec![ContextBlock::emit(
                "prompt",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::resource("input"),
            )])
    }
    fn config(strategy: &ContextStrategy, bindings: Value) -> Value {
        let source = context_source::generate(strategy).unwrap();
        use sha2::Digest;
        json!({"__zedflowVersion":3,"provider":"fixture","contextProgram":{"strategy":strategy,"source":source,"hash":format!("{:x}",sha2::Sha256::digest(source.as_bytes())),"types":{},"bindings":bindings},"fixtureSteps":[{"echoRequest":true}]})
    }
    fn prompt(output: &NodeOutput) -> String {
        let echoed: Value =
            serde_json::from_str(output.updates["output"].as_str().unwrap()).unwrap();
        echoed["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .into()
    }

    #[tokio::test]
    async fn actual_passage_captures_revision_retries_exactly_and_consumes_one_future_choice() {
        let f = Fixture::new().await;
        let mut cfg = config(
            &text_strategy(),
            json!({"input":{"kind":"state","field":"input"}}),
        );
        cfg["contextProgram"]["window"] = json!({"alias":"agent-window"});
        let original = f.run(&cfg, &f.ctx(0, "first")).await;
        assert_eq!(prompt(&original), "first");
        let snapshot = f.snapshot(&original).await;
        let revision: Revision =
            serde_json::from_value(snapshot["prepared"]["window"]["revision"].clone()).unwrap();
        assert_eq!(
            snapshot["prepared"]["window"]["origin"]["threadId"],
            "preparation/root"
        );
        assert_eq!(snapshot["prepared"]["window"]["origin"]["step"], 0);
        let registry = WindowRegistry::new(f.services.data_registry().unwrap());
        let edited = registry
            .patch_unique(
                &Scope::Flow("root".into()),
                "agent-window",
                &revision,
                &[WindowPatch::Representation {
                    id: "prompt".into(),
                    format: FragmentFormat::Text,
                    value: json!("edited explicitly"),
                }],
                "manual-edit",
            )
            .await
            .unwrap();
        let command = WindowSelectionCommand {
            id: uuid::Uuid::new_v4().to_string(),
            node_path: "root/agent".into(),
            alias: "agent-window".into(),
            revision: edited.revision.clone(),
            program_hash: crate::inference::program(&cfg["contextProgram"])
                .unwrap()
                .revision()
                .unwrap(),
        };
        window_preparation::queue_selection(&f.services, &command)
            .await
            .unwrap();
        window_preparation::queue_selection(&f.services, &command)
            .await
            .unwrap();
        let retry = f
            .run(
                &cfg,
                &f.ctx(0, "changed state cannot rewrite a captured passage"),
            )
            .await;
        assert_eq!(prompt(&retry), "first");
        let selected = f.run(&cfg, &f.ctx(1, "second")).await;
        assert_eq!(prompt(&selected), "edited explicitly");
        assert_eq!(
            f.snapshot(&selected).await["prepared"]["window"]["selectionCommandId"],
            command.id
        );
        let next = f.run(&cfg, &f.ctx(2, "third")).await;
        assert_eq!(prompt(&next), "third");
        assert_eq!(
            window::decode(
                registry
                    .revision(&Scope::Flow("root".into()), "agent-window", &revision)
                    .await
                    .unwrap()
                    .value
                    .as_ref()
            )
            .unwrap()
            .items
            .len(),
            1
        );
        assert_eq!(
            f.store
                .records(&f.services.id)
                .await
                .unwrap()
                .iter()
                .filter(|r| r.kind == "window-selection-used")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn window_selection_refuses_changed_linked_types_even_when_strategy_source_is_identical()
    {
        let f = Fixture::new().await;
        let mut original = config(
            &text_strategy(),
            json!({"input":{"kind":"state","field":"input"}}),
        );
        original["contextProgram"]["window"] = json!({"alias":"versioned-window"});
        let output = f
            .run(&original, &f.ctx(0, "captured before publication"))
            .await;
        let snapshot = f.snapshot(&output).await;
        let revision: Revision =
            serde_json::from_value(snapshot["prepared"]["window"]["revision"].clone()).unwrap();
        let program_hash = inference::program(&original["contextProgram"])
            .unwrap()
            .revision()
            .unwrap();
        window_preparation::queue_selection(
            &f.services,
            &WindowSelectionCommand {
                id: uuid::Uuid::new_v4().to_string(),
                node_path: "root/agent".into(),
                alias: "versioned-window".into(),
                revision,
                program_hash: program_hash.clone(),
            },
        )
        .await
        .unwrap();
        let mut next = original.clone();
        next["contextProgram"]["types"] = json!({"AdditionalDomainType":{"kind":"number"}});
        assert_eq!(
            next["contextProgram"]["hash"],
            original["contextProgram"]["hash"]
        );
        assert_ne!(
            inference::program(&next["contextProgram"])
                .unwrap()
                .revision()
                .unwrap(),
            program_hash
        );
        let node =
            models::node_with_services("agent", &next, "root/agent", f.services.clone()).unwrap();
        let error = node
            .execute(&f.ctx(1, "must not use obsolete prepared text"))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("no longer matches"), "{error}");
        assert_eq!(
            f.store
                .records(&f.services.id)
                .await
                .unwrap()
                .iter()
                .filter(|r| r.kind == "model-calls")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn window_capability_checks_frozen_alias_grant_and_recovers_started_patch_receipt() {
        let f = Fixture::new().await;
        let mut cfg = config(
            &text_strategy(),
            json!({"input":{"kind":"state","field":"input"}}),
        );
        cfg["contextProgram"]["window"] = json!({"alias":"agent-window"});
        let original = f.run(&cfg, &f.ctx(0, "original")).await;
        let raw = f.snapshot(&original).await;
        let revision: Revision =
            serde_json::from_value(raw["prepared"]["window"]["revision"].clone()).unwrap();
        let data = f.services.data_registry().unwrap();
        data.grant(
            &Scope::Flow("root".into()),
            "agent-window",
            &Scope::Flow("editor".into()),
            "target",
            Permission::Write,
        )
        .await
        .unwrap();
        let registry = WindowRegistry::new(data);
        let snapshot = EffectiveContext {
            invocation_id: uuid::Uuid::new_v4().to_string(),
            agent_path: "editor/agent".into(),
            origin: json!({"nodePath":"editor/agent","occurrenceId":"real-editor-passage"}),
            tools: vec!["context_window_patch".into()],
            system: String::new(),
            files: String::new(),
            resources: vec![],
            skill_catalog: vec![],
            prepared: Some(json!({"windowGrants":[{"alias":"target","permission":"write"}]})),
        };
        agent_capabilities::persist_snapshot(&f.services, &snapshot)
            .await
            .unwrap();
        let args = json!({"alias":"target","expectedRevision":revision,"patches":[{"kind":"representation","id":"prompt","format":"text","value":"editor revision"}]});
        let mut calls =
            vec![json!({"id":"provider-call","name":"context_window_patch","args":args})];
        agent_capabilities::seal_calls(&f.services, &snapshot, &mut calls)
            .await
            .unwrap();
        let (owner, call_id) = agent_capabilities::authorize_call(&f.services, &calls[0])
            .await
            .unwrap();
        let known = crate::resources::window::execute_tool(
            &f.services,
            &owner,
            &call_id,
            "context_window_patch",
            &args,
        )
        .await
        .unwrap();
        use sha2::Digest;
        let key = format!(
            "{:x}",
            sha2::Sha256::digest(format!("{}\0{owner}\0{call_id}", f.services.id))
        );
        f.store
            .put_record(
                &f.services.id,
                "receipts",
                &key,
                &json!({"name":"context_window_patch","arguments":args,"status":"started"}),
            )
            .await
            .unwrap();
        let recovered = f
            .services
            .execute_tool(&owner, &call_id, "context_window_patch", args.clone())
            .await
            .unwrap();
        assert_eq!(recovered, known);
        let head = registry
            .read(&Scope::Flow("root".into()), "agent-window")
            .await
            .unwrap();
        assert_eq!(json!(head.revision), known["revision"]);
        assert!(
            crate::resources::window::execute_tool(
                &f.services,
                "intruder/agent",
                &call_id,
                "context_window_patch",
                &args
            )
            .await
            .is_err()
        );
        let mut forged = args;
        forged["alias"] = json!("other-target");
        assert!(
            crate::resources::window::execute_tool(
                &f.services,
                &owner,
                &call_id,
                "context_window_patch",
                &forged
            )
            .await
            .is_err()
        );
        assert_eq!(
            registry
                .read(&Scope::Flow("root".into()), "agent-window")
                .await
                .unwrap()
                .revision,
            head.revision
        );
    }

    struct ProducerHost {
        version: String,
        calls: Mutex<Vec<BranchInvocation>>,
        waiting: AtomicBool,
        fail: AtomicBool,
    }
    #[async_trait::async_trait]
    impl DynamicCapabilities for ProducerHost {
        fn tools(&self, _: &str) -> Vec<Value> {
            vec![]
        }
        async fn route_contract(&self, _: &str, _: &str, route: Option<&str>) -> Result<Value> {
            Ok(
                json!({"routeId":route,"mode":"callAwait","invocation":"context","targetHash":self.version,"input":{"kind":"text"},"output":{"kind":"text"}}),
            )
        }
        async fn invoke_branch(&self, call: BranchInvocation) -> Result<RouteOutcome> {
            self.calls.lock().unwrap().push(call.clone());
            if self.fail.load(Ordering::SeqCst) {
                anyhow::bail!("producer failed explicitly");
            }
            if self.waiting.swap(false, Ordering::SeqCst) {
                return Ok(RouteOutcome::Waiting {
                    visit_id: "producer-visit".into(),
                    thread_id: "native-child".into(),
                    wait: json!({"kind":"model_selection","nodePath":"producer/agent"}),
                });
            }
            Ok(RouteOutcome::Completed {
                visit_id: "producer-visit".into(),
                thread_id: "native-child".into(),
                result: if call.input == "empty" {
                    json!("")
                } else {
                    json!(format!("{}:{}", self.version, call.input.as_str().unwrap()))
                },
            })
        }
        async fn invoke(&self, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
            anyhow::bail!("not a tool route")
        }
    }
    fn producer_host(version: &str, waiting: bool) -> Arc<ProducerHost> {
        Arc::new(ProducerHost {
            version: version.into(),
            calls: Mutex::new(vec![]),
            waiting: AtomicBool::new(waiting),
            fail: AtomicBool::new(false),
        })
    }
    fn producer_config() -> Value {
        let strategy = text_strategy()
            .require("derived", zf_core::types::DataType::Text)
            .with_program(vec![ContextBlock::emit(
                "derived",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::resource("derived"),
            )]);
        config(
            &strategy,
            json!({"input":{"kind":"state","field":"input"},"derived":{"kind":"produced","producer":{"branch":"produce","routeId":"root/producer","input":{"kind":"resource","name":"input"}}}}),
        )
    }

    #[tokio::test]
    async fn reader_is_acquired_when_only_a_demanded_producer_needs_it_and_changed_source_invalidates_result()
     {
        let f = Fixture::new().await;
        let host = producer_host("derive", false);
        f.services.set_dynamic_capabilities(host.clone());
        let mut cfg = producer_config();
        cfg["contextProgram"]["bindings"]["input"] = json!({"kind":"reader","reader":"file.text","input":{"kind":"literal","value":{"path":"producer-input.md"}}});
        std::fs::write(f.services.cwd.join("producer-input.md"), "first").unwrap();
        let output = f.run(&cfg, &f.ctx(0, "not implicitly selected")).await;
        assert_eq!(prompt(&output), "derive:first");
        assert_eq!(host.calls.lock().unwrap().len(), 1);
        assert_eq!(
            prompt(&f.run(&cfg, &f.ctx(1, "same source")).await),
            "derive:first"
        );
        assert_eq!(
            host.calls.lock().unwrap().len(),
            1,
            "Durable result reuses exact source identity"
        );
        std::fs::write(f.services.cwd.join("producer-input.md"), "second").unwrap();
        assert_eq!(
            prompt(&f.run(&cfg, &f.ctx(2, "changed source")).await),
            "derive:second"
        );
        assert_eq!(host.calls.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn producers_are_demanded_lazily_cache_empty_values_and_invalidate_inputs_and_target_revision()
     {
        let f = Fixture::new().await;
        let host = producer_host("v1", false);
        f.services.set_dynamic_capabilities(host.clone());
        let cfg = producer_config();
        assert_eq!(prompt(&f.run(&cfg, &f.ctx(0, "a")).await), "v1:a");
        assert_eq!(prompt(&f.run(&cfg, &f.ctx(1, "a")).await), "v1:a");
        assert_eq!(host.calls.lock().unwrap().len(), 1);
        assert_eq!(prompt(&f.run(&cfg, &f.ctx(2, "empty")).await), "");
        assert_eq!(prompt(&f.run(&cfg, &f.ctx(3, "empty")).await), "");
        assert_eq!(host.calls.lock().unwrap().len(), 2);
        let v2 = producer_host("v2", false);
        f.services.set_dynamic_capabilities(v2.clone());
        assert_eq!(prompt(&f.run(&cfg, &f.ctx(4, "a")).await), "v2:a");
        assert_eq!(v2.calls.lock().unwrap().len(), 1);
        let mut inactive = producer_config();
        let strategy = text_strategy().require("derived", zf_core::types::DataType::Text);
        inactive["contextProgram"] =
            config(&strategy, inactive["contextProgram"]["bindings"].clone())["contextProgram"]
                .clone();
        assert_eq!(
            prompt(&f.run(&inactive, &f.ctx(5, "inactive")).await),
            "inactive"
        );
        assert_eq!(v2.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn producer_wait_resumes_same_identity_and_never_calls_model_before_result() {
        let f = Fixture::new().await;
        let host = producer_host("v1", true);
        f.services.set_dynamic_capabilities(host.clone());
        let cfg = producer_config();
        let ctx = f.ctx(7, "question");
        let waiting = f.run(&cfg, &ctx).await;
        assert!(waiting.interrupt.is_some());
        assert_eq!(
            waiting.updates["contextNeeds"]["kind"],
            "context_production"
        );
        assert!(!waiting.updates.contains_key("modelResponse"));
        assert_eq!(prompt(&f.run(&cfg, &ctx).await), "v1:question");
        let calls = host.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].call_id, calls[1].call_id);
        assert_eq!(calls[0].input, calls[1].input);
        assert_eq!(
            calls[0].invocation,
            zf_flows::composition::InvocationKind::Context
        );
    }

    #[tokio::test]
    async fn active_producer_cycles_and_incompatible_contracts_fail_before_any_effect() {
        let f = Fixture::new().await;
        let host = producer_host("v1", false);
        f.services.set_dynamic_capabilities(host.clone());
        let mut cfg = producer_config();
        cfg["contextProgram"]["bindings"]["input"] = json!({"kind":"produced","producer":{"branch":"produce","routeId":"root/producer","input":{"kind":"resource","name":"derived"}}});
        let program = inference::program(&cfg["contextProgram"]).unwrap();
        let error = inference::prepare_at(
            &cfg,
            &program,
            &f.services,
            "root/agent",
            &f.ctx(0, "unused"),
            "fixture",
        )
        .await
        .err()
        .unwrap();
        assert!(error.to_string().contains("producer_cycle"));
        assert!(host.calls.lock().unwrap().is_empty());
        let mut mismatch = producer_config();
        mismatch["contextProgram"]["bindings"]["derived"]["producer"]["input"] =
            json!({"kind":"literal","dataType":{"kind":"number"},"value":123});
        // Literal syntax is decoded by the typed AST; the route's Text input cannot coerce it.
        let program = inference::program(&mismatch["contextProgram"]).unwrap();
        assert!(
            inference::prepare_at(
                &mismatch,
                &program,
                &f.services,
                "root/agent",
                &f.ctx(1, "unused"),
                "fixture"
            )
            .await
            .is_err()
        );
        assert!(host.calls.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod request_preview_tests {
    use super::request_preview::{PreviewProfile, prepare};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeMap, sync::Arc};
    use zf_context::{
        context::{self, ContextBlock, ContextExpr, ContextStrategy, FragmentFormat, FragmentRole},
        request_preview::program,
    };
    use zf_core::types::DataType;

    #[tokio::test]
    async fn trial_media_requires_supplied_bytes_and_preserves_raw_identity() {
        let strategy = ContextStrategy::new_v2("image-trial", "Image")
            .require(
                "image",
                DataType::Media {
                    media_type: "image/png".into(),
                },
            )
            .with_program(vec![ContextBlock::emit(
                "image",
                FragmentRole::Data,
                FragmentFormat::Media,
                ContextExpr::resource("image"),
            )]);
        let program = program(
            strategy,
            String::new(),
            String::new(),
            BTreeMap::new(),
            Default::default(),
        );
        let resources = BTreeMap::from([(
            "image".into(),
            Arc::new(json!({"mediaType":"image/png","contentRef":"explicit-image"})),
        )]);
        let evaluation = context::evaluate_with_library(
            &program.strategy,
            &resources,
            &program.types,
            &program.library,
        );
        let mut profile: PreviewProfile =
            serde_json::from_value(json!({"provider":"fixture","model":"fixture","config":{}}))
                .unwrap();
        let missing = prepare(&profile, &program, &evaluation, &resources).await;
        assert_eq!(missing["status"], "trial");
        assert!(
            missing["diagnostics"][0]["message"]
                .as_str()
                .unwrap()
                .contains("explicit-image")
        );
        profile.media.insert(
            "explicit-image".into(),
            json!({"encoding":"base64","byteLength":3,"chunks":["AQID"]}),
        );
        let prepared = prepare(&profile, &program, &evaluation, &resources).await;
        assert_eq!(prepared["status"], "prepared", "{prepared}");
        let raw = prepared["raw"].as_str().unwrap();
        assert_eq!(
            prepared["sha256"],
            format!("{:x}", Sha256::digest(raw.as_bytes()))
        );
        assert_eq!(prepared["byteLength"], raw.len());
        let request: Value = serde_json::from_str(raw).unwrap();
        let contents: Vec<adk_core::Content> =
            serde_json::from_value(request["contents"].clone()).unwrap();
        assert!(
            matches!(&contents[0].parts[0], adk_core::Part::InlineData { data, mime_type, .. } if data == &[1,2,3] && mime_type == "image/png")
        );
    }

    #[test]
    fn trial_profile_rejects_unknown_generation_settings_and_preserves_explicit_options() {
        assert!(serde_json::from_value::<PreviewProfile>(json!({"provider":"fixture","model":"fixture","config":{"unsupportedParameter":true}})).is_err());
        assert!(
            serde_json::from_value::<PreviewProfile>(
                json!({"provider":"fixture","model":"fixture","config":null})
            )
            .is_err()
        );
        let profile: PreviewProfile = serde_json::from_value(json!({"provider":"codex","model":"test","config":{"temperature":0.25},"reasoningEffort":"high","reasoningSummary":"auto","textVerbosity":"low"})).unwrap();
        assert_eq!(profile.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(profile.reasoning_summary.as_deref(), Some("auto"));
        assert_eq!(profile.text_verbosity.as_deref(), Some("low"));
        assert_eq!(
            serde_json::to_value(profile.config).unwrap()["temperature"],
            json!(0.25)
        );
    }
}
