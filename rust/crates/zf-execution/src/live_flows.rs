//! Authoring preflight for a flow shared by several workspaces and live runs.
//! No node is executed here. Accepted source bytes and run publications are
//! joined by source_acceptance's durable handoff.
use crate::{live_files, sources};
use anyhow::{Context, Result, ensure};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use zf_compiler::{
    graph_compiler::{self, GraphValidator},
    prepared, programs, resolve as composition,
};
use zf_flows::{
    composition::{CompositionCatalog, ResolveRequest},
    flow_contract,
    schema::Composition,
};
use zf_runtime::{
    materialize::RuntimePrimitives,
    revisions::{self, RevisionDefinition, RevisionPublication},
};
use zf_storage::{
    bridge_store::BridgeStore,
    content_store::ContentStore,
    flow_store::{FlowFile, FlowStore, FlowWrite},
    source_acceptance::{self, FilePrecondition},
    workspaces::{self, Workspace},
};

pub async fn accept(
    db: &SqlitePool,
    store: &ContentStore,
    flows: &FlowStore,
    selected: &Workspace,
    plan: FlowWrite,
    expected_hash: Option<String>,
    global: bool,
) -> Result<FlowFile> {
    let flow_key = workspaces::path_id(&plan.path);
    let workspaces = if global {
        workspaces::list(db).await?
    } else {
        vec![selected.clone()]
    };
    let mut publications = Vec::new();
    let mut preconditions = BTreeMap::new();
    for workspace in workspaces {
        live_files::recover(db, &workspace.path).await?;
        let files = flows.list(&workspace).await?;
        let affected: Vec<_> = live_files::run_definitions(db, &workspace.id)
            .await?
            .into_iter()
            .filter(|run| run.definition.key == flow_key)
            .collect();
        validate_dependents(
            &workspace,
            &files,
            &flow_key,
            &plan.composition,
            &mut preconditions,
        )
        .await?;
        // Mere visibility of a global flow does not select its workspace-local
        // context catalogue. Link the current editing workspace and every run
        // that actually uses it; unused workspaces keep their own diagnostics.
        if workspace.id != selected.id && affected.is_empty() {
            continue;
        }
        // A global definition is compiled against each workspace's own sources.
        let mut linked = plan.composition.clone();
        let dependencies = programs::freeze(
            &mut linked,
            &sources::program_sources(&plan.composition, &workspace.path, &[]).await?,
        )
        .with_context(|| {
            format!(
                "Workspace {}: contexte du flow incompatible",
                workspace.name
            )
        })?;
        graph_compiler::validate(&linked, &RuntimePrimitives)?;
        for condition in sources::captured_sources(&linked, &workspace.path)? {
            if let Some(prior) = preconditions.insert(condition.path, condition.hash.clone()) {
                ensure!(
                    prior == condition.hash,
                    zf_storage::context_store::Conflict(
                        "Une source a changé pendant la préparation des dépendances"
                    )
                );
            }
        }
        let source = if dependencies.is_empty() {
            plan.source.clone()
        } else {
            zf_flows::flow_format::render(&linked, &GraphValidator::new(&RuntimePrimitives))?
        };
        let definition = RevisionDefinition {
            key: flow_key.clone(),
            hash: zf_storage::flow_store::hash(source.as_bytes()),
            source,
            composition: linked,
        };
        for run in affected {
            let definition = if run.context_selections.is_empty() {
                definition.clone()
            } else {
                let mut composition = plan.composition.clone();
                prepared::apply_context_selections(
                    &mut composition,
                    &run.context_selections,
                    Some(&run.definition.composition),
                )?;
                let captured = sources::program_sources(&composition, &workspace.path, &[]).await?;
                programs::freeze(&mut composition, &captured)?;
                graph_compiler::validate(&composition, &RuntimePrimitives)?;
                for condition in sources::captured_sources(&composition, &workspace.path)? {
                    if let Some(prior) =
                        preconditions.insert(condition.path, condition.hash.clone())
                    {
                        ensure!(
                            prior == condition.hash,
                            zf_storage::context_store::Conflict(
                                "Une source a changé pendant la préparation des instances"
                            )
                        );
                    }
                }
                let source = zf_flows::flow_format::render(
                    &composition,
                    &GraphValidator::new(&RuntimePrimitives),
                )?;
                RevisionDefinition {
                    key: flow_key.clone(),
                    hash: zf_storage::flow_store::hash(source.as_bytes()),
                    source,
                    composition,
                }
            };
            let compatibility = revisions::compatibility(&run.baseline, &definition.composition)?;
            ensure!(
                compatibility.can_publish(),
                "Run {}, instance {} : enregistrement incompatible ({compatibility:?})",
                run.run_id,
                run.instance
            );
            publications.push(RevisionPublication {
                run_id: run.run_id,
                instance: run.instance,
                baseline: run.baseline,
                definition: definition.clone(),
            });
        }
    }
    // The source itself is checked by expected_hash, including creation.
    preconditions.remove(&plan.path);
    let publication = live_files::stage_publications(store, &publications).await?;
    let pending = source_acceptance::begin_flow(
        plan.path.clone(),
        plan.lock_workspace,
        plan.source,
        expected_hash,
        Some(publication),
        preconditions
            .into_iter()
            .map(|(path, hash)| FilePrecondition { path, hash })
            .collect(),
    )
    .await?;
    live_files::finish(store, pending).await?;
    flows.get(selected, &flow_key).await
}

fn catalog(
    files: &[FlowFile],
    replacement: Option<(&str, &Composition)>,
) -> Result<CompositionCatalog> {
    let mut result = CompositionCatalog::default();
    for file in files {
        let doc = replacement
            .filter(|(key, _)| *key == file.key)
            .map(|(_, doc)| doc)
            .or(file.composition.as_ref());
        let Some(doc) = doc else { continue };
        let Some(exports) = flow_contract::validate(doc)? else {
            continue;
        };
        for (name, ty) in exports.types {
            if let Some(old) = result.types.insert(name.clone(), ty.clone()) {
                ensure!(old == ty, "Conflicting shared type {name}");
            }
        }
        result.flows.insert(file.key.clone(), exports.contract);
    }
    Ok(result)
}

/// Existing valid bridge contracts must remain resolvable. An unrelated invalid
/// draft does not prevent editing a flow that it does not use.
async fn validate_dependents(
    workspace: &Workspace,
    files: &[FlowFile],
    key: &str,
    candidate: &Composition,
    preconditions: &mut BTreeMap<std::path::PathBuf, String>,
) -> Result<()> {
    let mut before = catalog(files, None)?;
    let mut after = catalog(files, Some((key, candidate)))?;
    for file in files {
        if !file.hash.is_empty() {
            preconditions.insert(file.path.clone(), file.hash.clone());
        }
    }
    for file in BridgeStore::new(workspace.path.clone())?.list().await? {
        if !file.hash.is_empty() {
            preconditions.insert(file.path, file.hash);
        }
        if let Some(bridge) = file.bridge {
            before.bridges.insert(file.key, bridge);
        }
    }
    after.bridges.clone_from(&before.bridges);
    for bridge in before.bridges.keys() {
        for (flow, contract) in &before.flows {
            for entry in contract.entries.keys() {
                let request = ResolveRequest {
                    flow: flow.clone(),
                    entry: entry.clone(),
                    bridges: vec![bridge.clone()],
                };
                if let Ok(previous) = composition::resolve(&before, &request)
                    && previous
                        .instances
                        .values()
                        .any(|instance| instance.flow == key)
                {
                    composition::resolve(&after,&request).map_err(|errors|anyhow::anyhow!(
                        "Bridge {bridge}, entrée {flow}/{entry} : le flow modifié invalide cette composition ({errors:?})"))?;
                }
            }
        }
    }
    Ok(())
}
