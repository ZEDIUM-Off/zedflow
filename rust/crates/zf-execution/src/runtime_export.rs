//! Portable entrypoint for an explicitly resolved, source-pinned ADK composition.
//! This module is copied into Cargo exports; it has no daemon or catalog loader.
use crate::route_runtime::{NativeFactory, RouteRuntime};
use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer, error::GraphError};
use anyhow::{Context, Result, ensure};
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

/// Run the same graph, registry, native route host and receipt/checkpoint stores
/// as the daemon. Reusing a run ID resumes its frontier, not its side effects.
pub async fn run(
    prepared: PreparedRuntime,
    factories: BTreeMap<String, NativeFactory>,
    options: RunOptions,
) -> Result<Value> {
    prepared.validate(&RuntimePrimitives)?;
    let workspace = tokio::fs::canonicalize(&options.workspace).await?;
    let run_data = options.data.join(&options.run_id);
    tokio::fs::create_dir_all(&run_data).await?;
    let source_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&prepared)?));
    let identity = json!({"workspace":workspace,"runtimeHash":source_hash});
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
    services.set_revisions(
        RevisionRuntime::new(content.clone(), &options.run_id, definitions).await?,
    )?;
    for (path, ids) in capabilities {
        services.set_active_capabilities(path, ids);
    }
    let (sender, mut receiver) = zf_runtime::event_sink::channel(64);
    services.set_sender(Some(sender.clone()));
    let event_file = run_data.join("events.jsonl");
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
    let checkpoint = Arc::new(
        StoredCheckpointer::new(CheckpointStore::new(content.clone()).await?)
            .with_sender(sender.clone()),
    );
    let entry = prepared.graph.entry.clone();
    let mut definition = RevisionDefinition::from_prepared(&prepared, &entry.instance)?;
    if let Some(saved) = checkpoint.load(&options.run_id).await?
        && let Some(selected) = zf_runtime::revisions::checkpoint_definition(
            &content,
            &options.run_id,
            &options.run_id,
            saved.step,
            &entry.instance,
        )
        .await?
    {
        ensure!(
            selected.key == definition.key,
            "Checkpoint belongs to another exported flow"
        );
        definition = selected;
    }
    let projection = flow_contract::at_entry(&definition.composition, &entry.port)?;
    let mut config = ExecutionConfig::new(&options.run_id)
        .with_recursion_limit(projection.settings.recursion_limit);
    let mut controller = services
        .revisions()
        .context("Revision controller absent")?
        .rebased(&entry.instance, definition.clone())
        .await?;
    let runtime = RouteRuntime::new(
        prepared,
        &services,
        checkpoint.clone(),
        Some(sender.clone()),
    )?;
    runtime.set_native_factories(factories)?;
    services.set_dynamic_capabilities(runtime.clone());
    runtime.initialize(&options.input).await?;
    let mut graph = runtime.build_instance_with_revisions(
        &entry.instance,
        &projection,
        Some(controller.clone()),
        Some(&definition.revision()),
    )?;
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
                request["scope"] == entry.instance && request["threadId"] == options.run_id,
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
                .rebased(&entry.instance, selected.clone())
                .await?;
            let projection = flow_contract::at_entry(&selected.composition, &entry.port)?;
            config = config.with_recursion_limit(projection.settings.recursion_limit);
            graph = runtime.build_instance_with_revisions(
                &entry.instance,
                &projection,
                Some(controller.clone()),
                Some(&selected.revision()),
            )?;
            services.emit(json!({"type":"revision_adopted","scope":entry.instance,"threadId":options.run_id,"step":saved.step,"hash":selected.hash,"definitionRevision":selected.revision(),"definitionRef":request["definitionRef"]})).await;
            definition = selected;
            continue;
        }
        break result;
    };
    let launched = runtime.drain().await?;
    let routes = runtime.result_snapshot().await?;
    let value = match result {
        Ok(completed) => Ok(
            json!({"status":"completed","runId":options.run_id,"state":completed.state,"routes":routes,"launched":launched}),
        ),
        Err(GraphError::Interrupted(wait)) => Ok(
            json!({"status":"waiting","runId":options.run_id,"checkpoint":wait.checkpoint_id,"interrupt":wait.interrupt,"routes":routes,"launched":launched}),
        ),
        Err(error) => Err(error.into()),
    };
    drop(graph);
    drop(runtime);
    services.set_sender(None);
    drop(services);
    drop(checkpoint);
    drop(sender);
    events.await.context("event log task failed")??;
    value
}
