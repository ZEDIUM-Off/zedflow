//! Preflight and recovery for accepted authoring changes and live definition heads.
//! Files contain the definitions; SQLite contains immutable execution revisions.
use crate::sources;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use zf_compiler::{
    graph_compiler::{self, GraphValidator},
    prepared, programs,
};
use zf_flows::schema::Composition;
use zf_runtime::{
    materialize::RuntimePrimitives,
    revisions::{self, RevisionDefinition, RevisionPublication, RuntimeGraphPublication},
};
use zf_storage::{
    content_store::ContentStore,
    context_store::SourceFile,
    live_files::{self as snapshots, RunDefinitionSnapshot},
    source_acceptance::{self, PendingAcceptance},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDefinition {
    pub run_id: String,
    pub instance: String,
    /// The actual compiled baseline remains necessary for live compatibility.
    pub baseline: Composition,
    /// Latest accepted head, or the initial frozen definition before first edit.
    pub definition: RevisionDefinition,
    #[serde(default)]
    pub context_selections: BTreeMap<String, prepared::ContextSelection>,
}

pub async fn stage_publications(
    store: &ContentStore,
    publications: &[RevisionPublication],
) -> Result<Value> {
    revisions::validate_publications(publications)?;
    Ok(json!({"publicationsRef":store.intern(&serde_json::to_value(publications)?).await?}))
}

pub async fn stage_mixed_publications(
    store: &ContentStore,
    publications: &[RevisionPublication],
    graphs: &[RuntimeGraphPublication],
) -> Result<Value> {
    revisions::validate_runtime_graph_publications(graphs)?;
    let mut result = stage_publications(store, publications).await?;
    result["runtimeGraphsRef"] = json!(store.intern(&serde_json::to_value(graphs)?).await?);
    Ok(result)
}

pub async fn finish(store: &ContentStore, pending: PendingAcceptance) -> Result<SourceFile> {
    publish_pending(store, pending.id(), pending.publication()).await?;
    pending.finish().await
}

pub async fn finish_package(
    store: &ContentStore,
    pending: zf_storage::flow_packages::PendingPackage,
) -> Result<zf_flows::package::PackageSnapshot> {
    publish_pending(store, pending.id(), pending.publication()).await?;
    pending.finish().await
}

async fn publish_pending(
    store: &ContentStore,
    id: &str,
    publication: Option<&Value>,
) -> Result<()> {
    if let Some(publication) = publication {
        let reference = publication["publicationsRef"]
            .as_str()
            .context("Source publication batch reference is absent")?;
        let batch: Vec<RevisionPublication> =
            serde_json::from_value(store.resolve(reference).await?)?;
        let graphs: Vec<RuntimeGraphPublication> = match publication["runtimeGraphsRef"].as_str() {
            Some(reference) => serde_json::from_value(store.resolve(reference).await?)?,
            None => vec![],
        };
        revisions::publish_mixed_unique(store, id, &batch, &graphs).await?;
    }
    Ok(())
}

pub async fn recover(db: &SqlitePool, workspace: &Path) -> Result<()> {
    zf_storage::flow_packages::recover_lifecycle(workspace.into()).await?;
    if let Some(pending) = source_acceptance::recover(workspace.into()).await? {
        finish(&ContentStore::from_pool(db.clone()), pending).await?;
    }
    if let Some(pending) = zf_storage::flow_packages::recover(workspace.into()).await? {
        finish_package(&ContentStore::from_pool(db.clone()), pending).await?;
    }
    Ok(())
}

/// Reads only execution definitions and their heads. Large message histories,
/// progress traces and context payloads are never hydrated during authoring.
pub async fn run_definitions(db: &SqlitePool, workspace_id: &str) -> Result<Vec<RunDefinition>> {
    definitions_from_snapshots(&snapshots::snapshots(db, workspace_id).await?)
}

/// Interpret one coherent storage capture without re-reading mutable heads.
pub fn definitions_from_snapshots(rows: &[RunDefinitionSnapshot]) -> Result<Vec<RunDefinition>> {
    let mut definitions = vec![];
    for run in rows {
        let id = &run.run_id;
        let runtime = run.runtime_graph.clone();
        let context_selections: BTreeMap<String, prepared::ContextSelection> = runtime
            .get("definitions")
            .and_then(|pins| pins.get("contextSelections"))
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        let initial: std::collections::BTreeMap<String, RevisionDefinition> = if !runtime.is_null()
        {
            let runtime: prepared::PreparedRuntime = serde_json::from_value(runtime)?;
            runtime
                .flows
                .keys()
                .map(|instance| {
                    Ok((
                        instance.clone(),
                        RevisionDefinition::from_prepared(&runtime, instance)?,
                    ))
                })
                .collect::<Result<_>>()?
        } else {
            let composition: Composition = serde_json::from_value(run.composition.clone())?;
            let source = &run.flow_source;
            let source = source
                .as_str()
                .context("Run frozen source absent")?
                .to_owned();
            std::collections::BTreeMap::from([(
                String::new(),
                RevisionDefinition {
                    package: (!run.flow_package.is_null())
                        .then(|| serde_json::from_value(run.flow_package.clone()))
                        .transpose()?,
                    context_selections: BTreeMap::new(),
                    key: run.flow_ref["key"]
                        .as_str()
                        .unwrap_or(&composition.id)
                        .into(),
                    hash: zf_storage::flow_store::hash(source.as_bytes()),
                    source,
                    composition,
                },
            )])
        };
        let instances: Vec<_> = initial.keys().cloned().collect();
        for (instance, base) in initial {
            let prefix = format!("{instance}/");
            let choices = context_selections
                .iter()
                .filter(|(path, _)| {
                    instances
                        .iter()
                        .filter(|id| path.starts_with(&format!("{id}/")))
                        .max_by_key(|id| id.len())
                        == Some(&instance)
                })
                .filter_map(|(path, choice)| {
                    path.strip_prefix(&prefix)
                        .map(|relative| (relative.into(), choice.clone()))
                })
                .collect();
            let key = zf_storage::context_store::hash(instance.as_bytes());
            let definition = if let Some(head) = run.revision_heads.get(&key) {
                ensure!(
                    head.head["instance"] == instance,
                    "Revision head instance mismatch"
                );
                serde_json::from_value(head.definition.clone())?
            } else {
                base.clone()
            };
            revisions::validate_definition(&definition)?;
            definitions.push(RunDefinition {
                run_id: id.clone(),
                instance,
                baseline: base.composition,
                definition,
                context_selections: choices,
            });
        }
    }
    Ok(definitions)
}

/// Preflight is done before taking the filesystem lock. Callers serialize all
/// authoring commands with the execution service's shared authoring mutex; the intent then
/// rechecks every source read so an external edit cannot invalidate the proposal.
pub async fn accept_source(
    db: &SqlitePool,
    flows: &zf_storage::flow_store::FlowStore,
    workspace: &zf_storage::workspaces::Workspace,
    candidate: programs::SourceOverride,
) -> Result<SourceFile> {
    match candidate.kind {
        programs::SourceKind::Strategy => {
            let strategy = zf_context::context_source::parse(&candidate.source)
                .map_err(|d| anyhow::anyhow!("{d:?}"))?;
            ensure!(
                strategy.id == candidate.key,
                "Strategy identity must match its source key"
            );
        }
        programs::SourceKind::Library => {
            zf_context::context_source::parse_library(&candidate.source)
                .map_err(|d| anyhow::anyhow!("{d:?}"))?;
        }
        programs::SourceKind::Types => {
            zf_context::context_source::parse_types(&candidate.source)
                .map_err(|d| anyhow::anyhow!("{d:?}"))?;
        }
    }
    recover(db, &workspace.path).await?;
    let files = flows.list(workspace).await?;
    let mut preconditions = BTreeMap::new();
    for file in &files {
        for condition in &file.preconditions {
            preconditions.insert(condition.path.clone(), condition.hash.clone());
        }
        let Some(doc) = &file.composition else {
            continue;
        };
        if programs::depends_on(doc, candidate.kind, &candidate.key) {
            let linked = link_candidate(doc, &workspace.path, &candidate)
                .await
                .with_context(|| {
                    format!("Flow {}: source {} incompatible", file.name, candidate.key)
                })?;
            capture_conditions(&linked, &workspace.path, &candidate, &mut preconditions)?;
        }
    }
    let mut publications = vec![];
    for run in run_definitions(db, &workspace.id).await? {
        if !programs::depends_on(&run.definition.composition, candidate.kind, &candidate.key) {
            continue;
        }
        let composition = link_candidate(&run.definition.composition, &workspace.path, &candidate)
            .await
            .with_context(|| {
                format!(
                    "Run {}, instance {}: source {} incompatible",
                    run.run_id, run.instance, candidate.key
                )
            })?;
        capture_conditions(
            &composition,
            &workspace.path,
            &candidate,
            &mut preconditions,
        )?;
        let source =
            zf_flows::flow_format::render(&composition, &GraphValidator::new(&RuntimePrimitives))?;
        publications.push(RevisionPublication {
            run_id: run.run_id,
            instance: run.instance,
            baseline: run.baseline,
            definition: RevisionDefinition {
                package: run.definition.package,
                context_selections: run.definition.context_selections,
                key: run.definition.key,
                hash: zf_storage::flow_store::hash(source.as_bytes()),
                source,
                composition,
            },
        });
    }
    let segments = match candidate.kind {
        programs::SourceKind::Strategy => vec!["context".into()],
        programs::SourceKind::Library => vec!["context".into(), "libraries".into()],
        programs::SourceKind::Types => vec!["types".into()],
    };
    let path = segments
        .iter()
        .fold(workspace.path.join(".zedflow"), |path, segment: &String| {
            path.join(segment)
        })
        .join(format!("{}.rs", candidate.key));
    preconditions.remove(&path);
    let store = ContentStore::from_pool(db.clone());
    let publication = stage_publications(&store, &publications).await?;
    let pending = source_acceptance::begin(
        workspace.path.clone(),
        segments,
        candidate.key,
        candidate.source,
        candidate.expected_hash,
        Some(publication),
        conditions(preconditions),
    )
    .await?;
    finish(&store, pending).await
}

fn conditions(values: BTreeMap<PathBuf, String>) -> Vec<source_acceptance::FilePrecondition> {
    values
        .into_iter()
        .map(|(path, hash)| source_acceptance::FilePrecondition { path, hash })
        .collect()
}
fn capture_conditions(
    doc: &Composition,
    workspace: &Path,
    candidate: &programs::SourceOverride,
    out: &mut BTreeMap<PathBuf, String>,
) -> Result<()> {
    let folder = match candidate.kind {
        programs::SourceKind::Strategy => ".zedflow/context",
        programs::SourceKind::Library => ".zedflow/context/libraries",
        programs::SourceKind::Types => ".zedflow/types",
    };
    let edited = workspace.join(folder).join(format!("{}.rs", candidate.key));
    for condition in sources::captured_sources(doc, workspace)? {
        if condition.path == edited {
            continue;
        }
        if let Some(previous) = out.insert(condition.path, condition.hash.clone()) {
            ensure!(
                previous == condition.hash,
                zf_storage::context_store::Conflict(
                    "A dependency changed while linking affected definitions"
                )
            );
        }
    }
    Ok(())
}
async fn link_candidate(
    doc: &Composition,
    workspace: &Path,
    candidate: &programs::SourceOverride,
) -> Result<Composition> {
    let mut composition = doc.clone();
    let overrides = std::slice::from_ref(candidate);
    let captured = sources::program_sources(&composition, workspace, overrides).await?;
    programs::freeze_with_overrides(&mut composition, &captured, overrides)?;
    graph_compiler::validate(&composition, &RuntimePrimitives)?;
    Ok(composition)
}

#[derive(Clone, Debug)]
pub struct RunGraph {
    pub run_id: String,
    pub baseline: prepared::PreparedRuntime,
    pub prepared: prepared::PreparedRuntime,
}
/// Fetch runtime composition headers and exact currently accepted flow heads.
/// The authoring mutex serializes all head-changing commands while this runs.
pub async fn run_graphs(db: &SqlitePool, workspace_id: &str) -> Result<Vec<RunGraph>> {
    graphs_from_snapshots(&snapshots::snapshots(db, workspace_id).await?)
}

/// Project graphs and flow heads from the same captured SQLite snapshot.
pub fn graphs_from_snapshots(rows: &[RunDefinitionSnapshot]) -> Result<Vec<RunGraph>> {
    let definitions = definitions_from_snapshots(rows)?;
    let mut graphs = vec![];
    for run in rows {
        let run_id = &run.run_id;
        if run.runtime_graph.is_null() {
            continue;
        }
        let baseline: prepared::PreparedRuntime =
            serde_json::from_value(run.runtime_graph.clone())?;
        let mut prepared = if let Some(head) = &run.runtime_graph_head {
            serde_json::from_value(head.definition.clone())?
        } else {
            baseline.clone()
        };
        for definition in definitions.iter().filter(|d| &d.run_id == run_id) {
            definition
                .definition
                .apply_to(&mut prepared, &definition.instance)?;
            let flow = &prepared.flows[&definition.instance];
            // Compatible flow edits may enrich inference metadata; the plan's
            // structural contract is rebuilt from these exact current flows.
            prepared
                .graph
                .instances
                .get_mut(&definition.instance)
                .context("Runtime instance absent")?
                .definition = flow.exports.contract.clone();
        }
        let catalog = zf_flows::composition::CompositionCatalog {
            types: prepared.graph.types.clone(),
            bridges: prepared.graph.bridges.clone(),
            flows: prepared
                .flows
                .values()
                .map(|f| (f.key.clone(), f.exports.contract.clone()))
                .collect(),
        };
        let request = graph_request(&prepared)?;
        prepared.graph = zf_compiler::resolve::resolve(&catalog, &request)
            .map_err(|d| anyhow::anyhow!("{d:?}"))?;
        prepared.validate(&RuntimePrimitives)?;
        graphs.push(RunGraph {
            run_id: run_id.clone(),
            baseline,
            prepared,
        });
    }
    Ok(graphs)
}
fn graph_request(
    prepared: &prepared::PreparedRuntime,
) -> Result<zf_flows::composition::ResolveRequest> {
    Ok(zf_flows::composition::ResolveRequest {
        flow: prepared
            .graph
            .instances
            .get(&prepared.graph.entry.instance)
            .context("Entry instance absent")?
            .flow
            .clone(),
        entry: prepared.graph.entry.port.clone(),
        bridges: prepared.graph.bridges.keys().cloned().collect(),
    })
}

pub async fn accept_bridge(
    db: &SqlitePool,
    flows: &zf_storage::flow_store::FlowStore,
    workspace: &zf_storage::workspaces::Workspace,
    key: &str,
    bridge: &zf_flows::composition::BridgeDefinition,
    expected_hash: Option<String>,
) -> Result<SourceFile> {
    use zf_compiler::resolve as composition;
    use zf_flows::{
        composition::{CompositionCatalog, ResolveRequest},
        flow_contract,
    };
    use zf_storage::bridge_store::BridgeStore;
    recover(db, &workspace.path).await?;
    let source = zf_flows::bridge_source::generate(bridge).map_err(|d| anyhow::anyhow!("{d:?}"))?;
    ensure!(
        serde_json::to_value(
            zf_flows::bridge_source::parse(&source).map_err(|d| anyhow::anyhow!("{d:?}"))?
        )? == serde_json::to_value(bridge)?,
        "Bridge source does not round-trip"
    );
    let mut preconditions = BTreeMap::new();
    let mut before = CompositionCatalog::default();
    for file in flows.list(workspace).await? {
        for condition in &file.preconditions {
            preconditions.insert(condition.path.clone(), condition.hash.clone());
        }
        let Some(doc) = file.composition else {
            continue;
        };
        let Some(exports) = flow_contract::validate(&doc)? else {
            continue;
        };
        for (name, ty) in exports.types {
            if let Some(old) = before.types.insert(name.clone(), ty.clone()) {
                ensure!(old == ty, "Conflicting shared type {name}");
            }
        }
        before.flows.insert(file.key, exports.contract);
    }
    for file in BridgeStore::new(workspace.path.clone())?.list().await? {
        if !file.hash.is_empty() {
            preconditions.insert(file.path, file.hash);
        }
        if let Some(bridge) = file.bridge {
            before.bridges.insert(file.key, bridge);
        }
    }
    let mut after = before.clone();
    after.bridges.insert(key.into(), bridge.clone());
    // Every previously valid composition involving the bridge must remain valid.
    // A new bridge must be usable from at least one declared flow entry.
    let mut usable = false;
    for (flow, contract) in &before.flows {
        for entry in contract.entries.keys() {
            for selected in after.bridges.keys() {
                let request = ResolveRequest {
                    flow: flow.clone(),
                    entry: entry.clone(),
                    bridges: vec![selected.clone()],
                };
                let was_used = composition::resolve(&before, &request)
                    .is_ok_and(|g| g.bridges.contains_key(key));
                let result = composition::resolve(&after, &request);
                usable |= result.as_ref().is_ok_and(|g| g.bridges.contains_key(key));
                if was_used {
                    result.map_err(|d| anyhow::anyhow!("Bridge {key}, {flow}/{entry}: {d:?}"))?;
                }
            }
        }
    }
    ensure!(
        usable,
        "Bridge {key} has no valid composition with the available flow entries"
    );
    let hash = zf_storage::context_store::hash(source.as_bytes());
    let mut graphs = vec![];
    for run in run_graphs(db, &workspace.id).await? {
        if !run.prepared.graph.bridges.contains_key(key) {
            continue;
        }
        let mut prepared = run.prepared;
        let request = graph_request(&prepared)?;
        let mut catalog = CompositionCatalog {
            types: prepared.graph.types.clone(),
            flows: prepared
                .flows
                .values()
                .map(|f| (f.key.clone(), f.exports.contract.clone()))
                .collect(),
            bridges: prepared.graph.bridges.clone(),
        };
        catalog.bridges.insert(key.into(), bridge.clone());
        // Existing runs retain every other accepted bridge definition. New
        // dependencies are explicit candidates and live compatibility checks
        // refuse any instance/alias/schema change requiring graph recompilation.
        for (name, definition) in &after.bridges {
            catalog
                .bridges
                .entry(name.clone())
                .or_insert_with(|| definition.clone());
        }
        prepared.graph = composition::resolve(&catalog, &request)
            .map_err(|d| anyhow::anyhow!("Run {}: {d:?}", run.run_id))?;
        prepared
            .definitions
            .bridge_sources
            .insert(key.into(), source.clone());
        prepared
            .definitions
            .bridge_hashes
            .insert(key.into(), hash.clone());
        graphs.push(RuntimeGraphPublication {
            run_id: run.run_id,
            baseline: run.baseline,
            prepared,
        });
    }
    let store = ContentStore::from_pool(db.clone());
    let publication = stage_mixed_publications(&store, &[], &graphs).await?;
    preconditions.remove(
        &workspace
            .path
            .join(".zedflow/bridges")
            .join(format!("{key}.rs")),
    );
    let pending = source_acceptance::begin(
        workspace.path.clone(),
        vec!["bridges".into()],
        key.into(),
        source,
        expected_hash,
        Some(publication),
        conditions(preconditions),
    )
    .await?;
    finish(&store, pending).await
}
