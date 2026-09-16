//! Host-side capture for composition preparation. The caller holds the shared
//! authoring lock and maintenance admission guard and recovers pending writes.
use crate::sources;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use zf_compiler::{
    prepared::{self, CompilationSnapshot, ContextSelection, PreparedRuntime},
    programs::{ProgramSources, SourceSnapshot},
    resolve,
};
use zf_flows::{
    composition::{CompositionCatalog, ResolveRequest},
    flow_contract,
};
use zf_runtime::{materialize::RuntimePrimitives, runtime::RunServices};
use zf_storage::{
    bridge_store::BridgeStore,
    flow_store::{Conflict, FlowStore},
    workspaces::Workspace,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSelection {
    pub flow: String,
    pub entry: String,
    #[serde(default)]
    pub bridges: Vec<String>,
    #[serde(default)]
    pub flow_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub bridge_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub contexts: BTreeMap<String, ContextSelection>,
}

/// Capture selected definitions and authenticated dependencies, compile them,
/// then reject edits observed during capture. This does not execute a flow.
///
/// # Errors
/// Returns source, resolution, stale-selection, or filesystem errors. The caller
/// must keep authoring serialized through admission of the returned runtime.
pub async fn prepare(
    flows: &FlowStore,
    workspace: &Workspace,
    selection: &RuntimeSelection,
) -> Result<PreparedRuntime> {
    let request = ResolveRequest {
        flow: selection.flow.clone(),
        entry: selection.entry.clone(),
        bridges: selection.bridges.clone(),
    };
    let bridge_store = BridgeStore::new(workspace.path.clone())?;
    let mut snapshot = CompilationSnapshot::default();
    let mut catalog = CompositionCatalog::default();
    let mut conditions = BTreeMap::new();
    let mut package_roots = BTreeMap::new();
    let mut definitions = BTreeMap::new();
    let mut discovery_hashes = BTreeMap::new();

    // list() intentionally omits bytes. Retain every valid public definition:
    // unselected flows can contribute shared types used by the selected graph.
    // Program catalogues, however, are acquired only after instance resolution.
    for listed in flows.list(workspace).await? {
        if !listed.diagnostics.is_empty() {
            continue;
        }
        let Some(doc) = &listed.composition else {
            continue;
        };
        if flow_contract::validate(doc)?.is_none() {
            continue;
        }
        let file = flows.get(workspace, &listed.key).await?;
        ensure!(
            file.diagnostics.is_empty(),
            "Flow capture contains diagnostics: {:?}",
            file.diagnostics
        );
        let Some(doc) = file.composition else {
            continue;
        };
        let Some(exports) = flow_contract::validate(&doc)? else {
            continue;
        };
        let source = file.source.context("Flow source is absent")?;
        let captured = SourceSnapshot::capture(source);
        ensure!(
            captured.hash == file.source_hash,
            "Flow capture hash mismatch"
        );
        for (name, ty) in exports.types {
            if let Some(previous) = catalog.types.insert(name.clone(), ty.clone()) {
                ensure!(previous == ty, "Conflicting shared type {name}");
            }
        }
        for condition in file.preconditions {
            if let Some(old) = conditions.insert(condition.path, condition.hash.clone()) {
                ensure!(
                    old == condition.hash,
                    Conflict("Package dependency changed during capture")
                );
            }
        }
        ensure!(
            listed.hash == file.hash,
            Conflict("Flow changed while resolving composition")
        );
        if let Some(package) = file.package {
            package_roots.insert(file.path.clone(), package.root.clone());
            snapshot.packages.insert(file.key.clone(), package);
        }
        discovery_hashes.insert(file.key.clone(), listed.hash);
        catalog.flows.insert(file.key.clone(), exports.contract);
        definitions.insert(file.key.clone(), doc);
        snapshot.flows.insert(file.key, captured);
    }
    let bridge_files = bridge_store.list().await?;
    for listed in &bridge_files {
        if let Some(bridge) = &listed.bridge {
            catalog.bridges.insert(listed.key.clone(), bridge.clone());
        }
    }
    let graph = resolve::resolve(&catalog, &request).map_err(|d| anyhow::anyhow!("{d:?}"))?;
    for key in graph.bridges.keys() {
        let listed = bridge_files
            .iter()
            .find(|file| &file.key == key)
            .context("Selected bridge source is absent")?;
        if let Some(expected) = selection.bridge_hashes.get(key) {
            ensure!(
                *expected == listed.hash,
                Conflict("Bridge changed since runtime graph selection")
            );
        }
        let fresh = bridge_store.read(key).await?;
        ensure!(
            fresh.hash == listed.hash,
            Conflict("Bridge changed while resolving composition")
        );
        let captured = SourceSnapshot::capture(fresh.source.context("Bridge source is absent")?);
        ensure!(captured.hash == fresh.hash, "Bridge capture hash mismatch");
        conditions.insert(fresh.path, captured.hash.clone());
        snapshot.bridges.insert(key.clone(), captured);
    }

    let mut overlays: BTreeMap<String, BTreeMap<String, ContextSelection>> = BTreeMap::new();
    for (path, selected) in &selection.contexts {
        let instance = graph
            .instances
            .keys()
            .filter(|instance| path.starts_with(&format!("{instance}/")))
            .max_by_key(|instance| instance.len())
            .with_context(|| format!("Context selection {path}: unknown flow instance"))?;
        overlays
            .entry(instance.clone())
            .or_default()
            .insert(path[instance.len() + 1..].into(), selected.clone());
    }
    for (instance, resolved) in &graph.instances {
        let captured = &snapshot.flows[&resolved.flow];
        let revision = snapshot
            .packages
            .get(&resolved.flow)
            .map_or(captured.hash.as_str(), |package| package.root.as_str());
        ensure!(
            discovery_hashes[&resolved.flow] == revision,
            Conflict("Flow changed while resolving composition")
        );
        if let Some(expected) = selection.flow_hashes.get(&resolved.flow) {
            ensure!(
                expected == revision,
                Conflict("Flow changed since runtime graph selection")
            );
        }
        let mut doc = definitions[&resolved.flow].clone();
        RunServices::validate_directories(&doc, &workspace.path).await?;
        if let Some(overlay) = overlays.get(instance) {
            prepared::apply_context_selections(&mut doc, overlay, None)?;
        }
        // Applying the overlay first avoids reading the replaced reference.
        merge_programs(
            &mut snapshot.programs,
            sources::program_sources(&doc, &workspace.path, &[]).await?,
        )?;
    }
    let runtime = prepared::prepare(
        &snapshot,
        &request,
        &selection.flow_hashes,
        &selection.contexts,
        &RuntimePrimitives,
    )?;
    for flow in runtime.flows.values() {
        for condition in sources::captured_sources(&flow.composition, &workspace.path)? {
            if let Some(previous) = conditions.insert(condition.path, condition.hash.clone()) {
                ensure!(
                    previous == condition.hash,
                    Conflict("A source changed while preparing runtime")
                );
            }
        }
    }
    recheck(&conditions).await?;
    for (path, revision) in package_roots {
        ensure!(
            zf_storage::flow_packages::capture(&path).await?.root == revision,
            Conflict("Flow package changed before compilation admission")
        );
    }
    Ok(runtime)
}

fn merge_programs(target: &mut ProgramSources, incoming: ProgramSources) -> Result<()> {
    for (catalog, captured) in [
        (&mut target.strategies, incoming.strategies),
        (&mut target.libraries, incoming.libraries),
        (&mut target.types, incoming.types),
    ] {
        for (key, source) in captured {
            if let Some(previous) = catalog.get_mut(&key) {
                ensure!(
                    previous.hash == source.hash && previous.source == source.source,
                    Conflict("A source changed while preparing runtime")
                );
                previous
                    .accepted_references
                    .extend(source.accepted_references);
            } else {
                catalog.insert(key, source);
            }
        }
    }
    Ok(())
}

pub(crate) async fn recheck(conditions: &BTreeMap<PathBuf, String>) -> Result<()> {
    for (path, hash) in conditions {
        let metadata = tokio::fs::symlink_metadata(path)
            .await
            .with_context(|| format!("Captured source unavailable: {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            Conflict("A captured source is no longer a regular file")
        );
        use tokio::io::AsyncReadExt;
        let limit = zf_flows::package::MAX_PACKAGE_FILE_BYTES;
        ensure!(
            metadata.len() <= limit as u64,
            "Captured source exceeds its byte limit"
        );
        let mut bytes = Vec::new();
        tokio::fs::File::open(path)
            .await?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(
            bytes.len() <= limit,
            "Captured source exceeds its byte limit"
        );
        ensure!(
            zf_storage::flow_store::hash(&bytes) == *hash,
            Conflict("A source changed while preparing runtime")
        );
    }
    Ok(())
}
