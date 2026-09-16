//! Portable entrypoint for an explicitly resolved, source-pinned ADK composition.
//! This module is copied into Cargo exports; it has no daemon or catalog loader.
use crate::route_runtime::{NativeFactory, RouteRuntime};
use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer, error::GraphError};
use anyhow::{Context, Result, ensure};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::io::AsyncWriteExt;
use zf_compiler::{
    graph_compiler::GraphValidator,
    prepared_model::{ContextSelection, DefinitionPins, FrozenFlow, PreparedRuntime},
    resolve::RuntimeGraph,
};
use zf_flows::{composition::BridgeDefinition, flow_contract};
use zf_runtime::{
    materialize::RuntimePrimitives,
    revisions::{RevisionDefinition, RevisionRuntime},
    runtime::RunServices,
    stored_checkpointer::StoredCheckpointer,
    workspace_context::ContextSnapshot,
};
use zf_storage::{content_store::ContentStore, contracts::CheckpointStore, data::DataRegistry};

/// Only runtime composition metadata lives outside the canonical Rust files.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportManifest {
    #[serde(default)]
    pub context_selections: BTreeMap<String, ContextSelection>,
    pub graph: RuntimeGraph,
    pub executed_hashes: BTreeMap<String, String>,
    pub flow_hashes: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub flow_packages: BTreeMap<String, zf_flows::package::PackageSnapshot>,
    pub bridge_hashes: BTreeMap<String, String>,
}

/// Reconstruct validated projections from the very source modules compiled into
/// the binary. No arbitrary Rust is interpreted or executed during this read.
pub fn assemble(
    manifest: ExportManifest,
    flows: Vec<(&str, &str, &str)>,
    bridges: Vec<(&str, &str, BridgeDefinition)>,
) -> Result<PreparedRuntime> {
    let mut frozen = BTreeMap::new();
    for (instance, key, source) in flows {
        let hash = format!("{:x}", Sha256::digest(source.as_bytes()));
        ensure!(
            manifest.executed_hashes.get(instance) == Some(&hash),
            "executed source hash mismatch: {instance}"
        );
        let composition =
            zf_flows::flow_format::parse(source, &GraphValidator::new(&RuntimePrimitives))?;
        let exports =
            flow_contract::validate(&composition)?.context("exported flow has no public ports")?;
        ensure!(
            frozen
                .insert(
                    instance.into(),
                    FrozenFlow {
                        key: key.into(),
                        hash,
                        source: source.into(),
                        composition,
                        exports
                    }
                )
                .is_none(),
            "duplicate exported instance: {instance}"
        );
    }
    let mut bridge_sources = BTreeMap::new();
    for (key, source, compiled) in bridges {
        ensure!(
            manifest
                .graph
                .bridges
                .get(key)
                .is_some_and(|bridge| serde_json::to_value(bridge).ok()
                    == serde_json::to_value(&compiled).ok()),
            "compiled bridge differs from prepared routes: {key}"
        );
        ensure!(
            bridge_sources.insert(key.into(), source.into()).is_none(),
            "duplicate exported bridge: {key}"
        );
    }
    let prepared = PreparedRuntime {
        graph: manifest.graph,
        flows: frozen,
        definitions: DefinitionPins {
            context_selections: manifest.context_selections,
            flow_hashes: manifest.flow_hashes,
            flow_packages: manifest.flow_packages,
            bridge_hashes: manifest.bridge_hashes,
            bridge_sources,
        },
    };
    prepared.validate(&RuntimePrimitives)?;
    Ok(prepared)
}

#[derive(Debug)]
pub struct RunOptions {
    pub workspace: PathBuf,
    pub data: PathBuf,
    pub run_id: String,
    pub input: State,
    pub models: Option<Value>,
    pub capabilities: Option<BTreeMap<String, Vec<String>>>,
}
impl RunOptions {
    pub async fn from_args() -> Result<Self> {
        let mut result = Self {
            workspace: std::env::current_dir()?,
            data: std::env::temp_dir().join("zedflow-composed"),
            run_id: uuid::Uuid::new_v4().to_string(),
            input: State::new(),
            models: None,
            capabilities: None,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .with_context(|| format!("{arg} requires a value"))?;
            match arg.as_str() {
                "--workspace" => result.workspace = value.into(),
                "--data" => result.data = value.into(),
                "--run-id" => result.run_id = value,
                "--input" => {
                    result.input = zf_context::context_json::from_str(&value)
                        .context("--input must be a JSON state object")?
                }
                "--models" => result.models = Some(json_argument(value).await?),
                "--capabilities" => {
                    result.capabilities = Some(serde_json::from_value(json_argument(value).await?)?)
                }
                _ => anyhow::bail!("unknown option: {arg}"),
            }
        }
        ensure!(
            !result.run_id.is_empty()
                && result.run_id != "."
                && result.run_id != ".."
                && !result.run_id.contains(['/', '\\']),
            "--run-id must be a directory-safe identifier"
        );
        Ok(result)
    }
}
async fn json_argument(value: String) -> Result<Value> {
    if value.trim_start().starts_with('{') {
        Ok(zf_context::context_json::from_str(&value)?)
    } else {
        Ok(zf_context::context_json::from_slice(
            &tokio::fs::read(value).await?,
        )?)
    }
}
async fn read_optional(path: &std::path::Path) -> Result<Option<Value>> {
    match tokio::fs::read(path).await {
        Ok(bytes) => Ok(Some(zf_context::context_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Run a validated composition with its source-pinned native factories.
/// The owned task retains the data lease until execution and observation drain,
/// even when its caller stops awaiting the result.
pub async fn run(
    prepared: PreparedRuntime,
    factories: BTreeMap<String, NativeFactory>,
    options: RunOptions,
) -> Result<Value> {
    prepared.validate(&RuntimePrimitives)?;
    let definition = RevisionDefinition::from_prepared(&prepared, &prepared.graph.entry.instance)?;
    let entry = prepared.graph.entry.clone();
    let definitions = prepared
        .flows
        .keys()
        .map(|instance| {
            Ok((
                instance.clone(),
                RevisionDefinition::from_prepared(&prepared, instance)?,
            ))
        })
        .collect::<Result<_>>()?;
    let source_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&prepared)?));
    tokio::spawn(run_owned(
        ExportRun {
            source_hash,
            definition,
            definitions,
            scope: entry.instance,
            port: Some(entry.port),
            prepared: Some(prepared),
            factories,
        },
        options,
    ))
    .await
    .context("export execution task failed")?
}

/// Run a standalone flow without manufacturing public ports or rewriting its
/// source. Resuming an ID restores the same receipts and ADK checkpoints.
pub async fn run_single(
    definition: RevisionDefinition,
    factory: NativeFactory,
    options: RunOptions,
) -> Result<Value> {
    zf_runtime::revisions::validate_definition(&definition)?;
    let source_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&definition)?));
    let definitions = BTreeMap::from([(String::new(), definition.clone())]);
    tokio::spawn(run_owned(
        ExportRun {
            source_hash,
            definition,
            definitions,
            scope: String::new(),
            port: None,
            prepared: None,
            factories: BTreeMap::from([(String::new(), factory)]),
        },
        options,
    ))
    .await
    .context("export execution task failed")?
}

struct ExportRun {
    source_hash: String,
    definition: RevisionDefinition,
    definitions: BTreeMap<String, RevisionDefinition>,
    scope: String,
    port: Option<String>,
    prepared: Option<PreparedRuntime>,
    factories: BTreeMap<String, NativeFactory>,
}
impl ExportRun {
    fn projection(&self, definition: &RevisionDefinition) -> Result<zf_flows::schema::Composition> {
        match &self.port {
            Some(port) => flow_contract::at_entry(&definition.composition, port),
            None => Ok(definition.composition.clone()),
        }
    }
    fn build(
        &self,
        definition: &RevisionDefinition,
        runtime: Option<&Arc<RouteRuntime>>,
        services: &Arc<RunServices>,
        checkpoint: &Arc<StoredCheckpointer>,
        controller: Arc<RevisionRuntime>,
        sender: zf_runtime::event_sink::EventSink,
    ) -> Result<adk_graph::CompiledGraph> {
        let projection = self.projection(definition)?;
        if let Some(runtime) = runtime {
            return runtime.build_instance_with_revisions(
                &self.scope,
                &projection,
                Some(controller),
                Some(&definition.revision()),
            );
        }
        let native = if definition.revision() == self.definition.revision() {
            Some(self
                .factories
                .get("")
                .context("standalone native factory absent")?(
                services.clone(),
                checkpoint.clone(),
                "",
            )?)
        } else {
            None
        };
        zf_runtime::materialize::build_scope_with_native_and_revisions(
            &projection,
            Some(sender),
            "",
            Some(services.clone()),
            Some(checkpoint.clone()),
            native.as_ref(),
            Some(controller),
        )
    }
}

async fn run_owned(mut export: ExportRun, options: RunOptions) -> Result<Value> {
    ensure!(
        !options.run_id.is_empty()
            && options.run_id != "."
            && options.run_id != ".."
            && !options.run_id.contains(['/', '\\', '\0']),
        "--run-id must be a directory-safe identifier"
    );
    tokio::fs::create_dir_all(&options.data).await?;
    let _lease = zf_storage::migration::lock(&tokio::fs::canonicalize(&options.data).await?)?;
    let workspace = tokio::fs::canonicalize(&options.workspace).await?;
    let run_data = options.data.join(&options.run_id);
    tokio::fs::create_dir_all(&run_data).await?;
    let identity = json!({"workspace":workspace,"runtimeHash":export.source_hash});
    let identity_file = run_data.join("runtime.json");
    if let Some(prior) = read_optional(&identity_file).await? {
        ensure!(
            prior == identity,
            "run belongs to a different workspace or frozen runtime"
        );
    } else {
        tokio::fs::write(identity_file, serde_json::to_vec_pretty(&identity)?).await?;
    }
    let context_file = run_data.join("context.json");
    let context = if let Some(value) = read_optional(&context_file).await? {
        serde_json::from_value(value)?
    } else {
        let value = ContextSnapshot::load(&workspace, &[]).await?;
        tokio::fs::write(context_file, serde_json::to_vec_pretty(&value)?).await?;
        value
    };
    let models_file = run_data.join("models.json");
    let mut models = read_optional(&models_file).await?.unwrap_or(json!({}));
    if let Some(additions) = options.models {
        models
            .as_object_mut()
            .context("stored model bindings are not an object")?
            .extend(
                additions
                    .as_object()
                    .context("model bindings must be an object")?
                    .clone(),
            );
    }
    tokio::fs::write(models_file, serde_json::to_vec_pretty(&models)?).await?;
    let capabilities_file = run_data.join("capabilities.json");
    let mut capabilities: BTreeMap<String, Vec<String>> = serde_json::from_value(
        read_optional(&capabilities_file)
            .await?
            .unwrap_or(json!({})),
    )?;
    if let Some(additions) = options.capabilities {
        capabilities.extend(additions);
    }
    tokio::fs::write(capabilities_file, serde_json::to_vec_pretty(&capabilities)?).await?;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(options.data.join("sessions.db"))
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
        )
        .await?;
    let content = ContentStore::new(pool).await?;
    let services = RunServices::new(
        options.run_id.clone(),
        workspace,
        run_data.clone(),
        context,
        models,
        vec![],
    )?;
    services.set_content_store(content.clone());
    services.set_data_registry(
        DataRegistry::new(content.pool().clone(), content.clone(), &options.run_id).await?,
    )?;
    services.set_revisions(
        RevisionRuntime::new(content.clone(), &options.run_id, export.definitions.clone()).await?,
    )?;
    for (path, ids) in capabilities {
        services.set_active_capabilities(path, ids);
    }
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    services.set_sender(Some(sender.clone()));
    let event_file = run_data.join("events.jsonl");
    let checkpoint = Arc::new(
        StoredCheckpointer::new(CheckpointStore::new(content.clone()).await?)
            .with_sender(sender.clone()),
    );
    let events = tokio::spawn(async move {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(event_file)
            .await?;
        while let Some(event) = receiver.recv().await {
            let mut line = serde_json::to_vec(&event)?;
            line.push(b'\n');
            file.write_all(&line).await?;
        }
        file.flush().await?;
        Ok::<_, anyhow::Error>(())
    });
    let mut runtime: Option<Arc<RouteRuntime>> = None;
    // Keep ownership and task cleanup outside the native code unwind boundary.
    let value: Result<Value> = std::panic::AssertUnwindSafe(async {
    let mut definition = export.definition.clone();
    if let Some(saved) = checkpoint.load(&options.run_id).await?
        && let Some(selected) = zf_runtime::revisions::checkpoint_definition(
            &content,
            &options.run_id,
            &options.run_id,
            saved.step,
            &export.scope,
        )
        .await?
    {
        ensure!(
            selected.key == definition.key,
            "Checkpoint belongs to another exported flow"
        );
        definition = selected;
    }
    let projection = export.projection(&definition)?;
    let mut config = ExecutionConfig::new(&options.run_id)
        .with_recursion_limit(projection.settings.recursion_limit);
    let mut controller = services
        .revisions()
        .context("Revision controller absent")?
        .rebased(&export.scope, definition.clone())
        .await?;
    if let Some(prepared) = export.prepared.take() {
        let routes = RouteRuntime::new(prepared, &services, checkpoint.clone(), Some(sender.clone()))?;
        routes.set_native_factories(std::mem::take(&mut export.factories))?;
        services.set_dynamic_capabilities(routes.clone());
        runtime = Some(routes);
        runtime.as_ref().context("route runtime absent")?.initialize(&options.input).await?;
    }
    let mut graph = export.build(&definition, runtime.as_ref(), &services, &checkpoint, controller.clone(), sender.clone())?;
    let mut input = options.input;
    let result = loop {
        let result = graph
            .invoke_detailed(std::mem::take(&mut input), config.clone())
            .await;
        if let Err(GraphError::Interrupted(interrupted)) = &result
            && let adk_graph::Interrupt::Dynamic {
                data: Some(request),
                ..
            } = &interrupted.interrupt
            && request["kind"] == "revision_boundary"
        {
            ensure!(
                request["scope"] == export.scope && request["threadId"] == options.run_id,
                "Revision boundary belongs to another exported frontier"
            );
            let saved = checkpoint
                .load(&options.run_id)
                .await?
                .context("Revision boundary checkpoint absent")?;
            ensure!(
                saved.thread_id == options.run_id
                    && saved.step
                        == request["step"].as_u64().context("Boundary step missing")? as usize
                    && saved.pending_nodes.len() == 1
                    && saved.pending_nodes[0]
                        == request["node"].as_str().context("Boundary node missing")?,
                "Revision boundary does not match the durable sequential frontier"
            );
            let selected: RevisionDefinition = serde_json::from_value(
                content
                    .resolve(
                        request["definitionRef"]
                            .as_str()
                            .context("Boundary definition missing")?,
                    )
                    .await?,
            )?;
            ensure!(
                selected.hash == request["toHash"]
                    && selected.key == definition.key
                    && request["toRevision"].as_str().map_or(
                        selected.package.is_none() && selected.context_selections.is_empty(),
                        |revision| revision == selected.revision()
                    ),
                "Revision boundary source identity mismatch"
            );
            zf_runtime::revisions::validate_definition(&selected)?;
            controller = controller
                .rebased(&export.scope, selected.clone())
                .await?;
            let projection = export.projection(&selected)?;
            config = config.with_recursion_limit(projection.settings.recursion_limit);
            graph = export.build(&selected, runtime.as_ref(), &services, &checkpoint, controller.clone(), sender.clone())?;
            services.emit(json!({"type":"revision_adopted","scope":export.scope,"threadId":options.run_id,"step":saved.step,"hash":selected.hash,"definitionRevision":selected.revision(),"definitionRef":request["definitionRef"]})).await;
            definition = selected;
            continue;
        }
        break result;
    };
    if matches!(&result, Err(error) if !matches!(error, GraphError::Interrupted(_))) {
        return Err(result.err().context("graph error absent")?.into());
    }
    let (launched, routes) = if let Some(runtime) = &runtime {
        (runtime.drain().await?, runtime.result_snapshot().await?)
    } else { (Vec::new(), Vec::new()) };
    match result {
        Ok(completed) => Ok(
            json!({"status":"completed","runId":options.run_id,"state":completed.state,"routes":routes,"launched":launched}),
        ),
        Err(GraphError::Interrupted(wait)) => Ok(
            json!({"status":"waiting","runId":options.run_id,"checkpoint":wait.checkpoint_id,"interrupt":wait.interrupt,"routes":routes,"launched":launched}),
        ),
        Err(error) => Err(error.into()),
    }
    }).catch_unwind().await.unwrap_or_else(|_| Err(anyhow::anyhow!("native export execution panicked")));
    let cleanup = if value.is_err() {
        match &runtime {
            Some(runtime) => runtime.cancel_and_drain().await,
            None => Ok(()),
        }
    } else {
        Ok(())
    };
    drop(runtime);
    services.set_sender(None);
    drop(services);
    drop(checkpoint);
    drop(sender);
    let log = events
        .await
        .context("event log task failed")
        .and_then(|result| result);
    cleanup?;
    log?;
    value
}
