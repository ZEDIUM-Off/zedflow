//! Explicit catalogue migrations. Existing run snapshots and revision heads are
//! deliberately outside these operations; no model or runtime node is invoked.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use zf_compiler::{package_sources::validate_package_sources, resolve};
use zf_flows::{
    bridge_source,
    composition::{CompositionCatalog, ResolveRequest},
};
use zf_storage::{
    bridge_store::BridgeStore,
    flow_packages::{
        self, BridgeMutation, LegacyRetirement, PackageConversion, PackageDeletion,
        PackagePrecondition,
    },
    flow_store::{FlowFile, FlowStore},
    workspaces::{self, Workspace},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedConsumer {
    pub workspace_id: String,
    pub key: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResult {
    pub old_key: String,
    pub new_key: String,
    pub old_revision: String,
    pub new_revision: String,
    pub flow: FlowFile,
    pub changed_consumers: Vec<ChangedConsumer>,
}

/// Convert a legacy source after the caller admits an authoring command and
/// takes the service's shared authoring lock. Only registered workspaces and the
/// configured home catalogue are within the consumer audit boundary.
///
/// # Errors
/// Rejects stale source, identity collisions, unfrozen filesystem inputs,
/// unreadable consumer catalogues, or any concurrent catalogue change.
pub async fn convert(
    db: &SqlitePool,
    flows: &FlowStore,
    selected: &Workspace,
    home: &Path,
    key: &str,
    expected_hash: &str,
) -> Result<ConversionResult> {
    let workspaces = participants(db, selected, home).await?;
    // Capture the *whole* decision inventory before resolving even the source.
    // The lifecycle transaction checks these under all participant locks; a new
    // consumer or collision appearing during preflight is therefore a conflict.
    let preconditions = flow_packages::capture_catalogue_preconditions(
        workspaces.iter().map(|w| w.path.clone()).collect(),
    )
    .await?;
    let plan = flows.plan_conversion(selected, key, expected_hash).await?;
    validate_package_sources(&plan.package)
        .context("La source historique dépend de fichiers non figés ; conversion refusée")?;
    let new_key = workspaces::path_id(&plan.target);
    let mut mutations = BTreeMap::new();
    let mut changed_consumers = Vec::new();
    let mut package_preconditions = BTreeMap::new();
    for workspace in &workspaces {
        let files = flows.list(workspace).await?;
        let visible = files.iter().any(|file| file.key == key);
        if visible {
            ensure!(
                files
                    .iter()
                    .all(|file| file.key == key || file.id != plan.legacy.id),
                "Workspace {} : identité de flow concurrente {}",
                workspace.name,
                plan.legacy.id
            );
        }
        // Invalid or incomplete package closures cannot establish the absence of
        // a filesystem reference to the retiring source.
        validate_no_dependency(&files, &plan.legacy.path, key, &mut package_preconditions).await?;
        let mut before = crate::live_flows::catalog(&files, None)?;
        let mut after = before.clone();
        if let Some(contract) = after.flows.remove(key) {
            ensure!(
                after.flows.insert(new_key.clone(), contract).is_none(),
                "Collision de clé du package"
            );
        }
        let bridges = BridgeStore::new(workspace.path.clone())?;
        for listed in bridges.list().await? {
            let file = bridges.read(&listed.key).await?;
            let bridge = file.bridge.context(format!(
                "Bridge {} : impossible de vérifier les références ({:?})",
                file.path.display(),
                file.diagnostics
            ))?;
            let mut replacement = bridge.clone();
            let affected = bridge.imports.values().any(|import| import.flow == key);
            if affected {
                ensure!(
                    visible,
                    "Bridge {} : référence au flow hors de son catalogue",
                    file.path.display()
                );
                let source = file.source.context("Source de bridge absente")?;
                let remapped = bridge_source::remap_flow_imports(&source, key, &new_key).map_err(
                    |errors| anyhow::anyhow!("Bridge {} : {errors:?}", file.path.display()),
                )?;
                replacement = bridge_source::parse(&remapped)
                    .map_err(|errors| anyhow::anyhow!("Bridge remappé invalide : {errors:?}"))?;
                if !mutations.contains_key(&file.path) {
                    changed_consumers.push(ChangedConsumer {
                        workspace_id: workspace.id.clone(),
                        key: file.key.clone(),
                        path: file.path.clone(),
                    });
                    mutations.insert(
                        file.path.clone(),
                        BridgeMutation {
                            workspace: workspace.path.clone(),
                            path: file.path,
                            before: source.into_bytes(),
                            after: remapped.into_bytes(),
                        },
                    );
                }
            }
            before.bridges.insert(file.key.clone(), bridge);
            after.bridges.insert(file.key, replacement);
        }
        validate_remapped_resolutions(&before, &after, key, &new_key).with_context(|| {
            format!(
                "Workspace {} : composition après conversion",
                workspace.name
            )
        })?;
    }
    let old_revision = plan.legacy.hash.clone();
    let snapshot = flow_packages::convert_package(PackageConversion {
        workspace: plan.lock_workspace,
        snapshot: plan.package,
        legacy: LegacyRetirement {
            path: plan.legacy.path,
            before: plan
                .legacy
                .source
                .context("Source historique absente")?
                .into_bytes(),
        },
        bridges: mutations.into_values().collect(),
        preconditions,
        package_preconditions: package_preconditions
            .into_iter()
            .map(|(path, revision)| PackagePrecondition { path, revision })
            .collect(),
    })
    .await?;
    let flow = flows.get(selected, &new_key).await?;
    ensure!(
        flow.hash == snapshot.root,
        "Le package a changé après conversion ; actualisez le catalogue"
    );
    Ok(ConversionResult {
        old_key: key.into(),
        new_key,
        old_revision,
        new_revision: snapshot.root,
        flow,
        changed_consumers,
    })
}

/// Delete a package only when no current bridge or package closure refers to it.
/// The caller must admit the command under the shared authoring lock. Historical
/// runs are not catalogue dependents and are never queried or modified here.
///
/// # Errors
/// Rejects legacy files, stale full revisions, references, unreadable dependency
/// inventories, and concurrent changes detected by the lifecycle transaction.
pub async fn delete(
    db: &SqlitePool,
    flows: &FlowStore,
    selected: &Workspace,
    home: &Path,
    key: &str,
    expected_hash: &str,
) -> Result<()> {
    let workspaces = participants(db, selected, home).await?;
    let preconditions = flow_packages::capture_catalogue_preconditions(
        workspaces.iter().map(|w| w.path.clone()).collect(),
    )
    .await?;
    let old = flows.get(selected, key).await?;
    ensure!(
        old.package.is_some(),
        "La suppression concerne un package de flow"
    );
    ensure!(
        old.hash == expected_hash && !expected_hash.is_empty(),
        zf_storage::flow_store::Conflict("Le package a changé avant sa suppression")
    );
    let mut package_preconditions = BTreeMap::new();
    for workspace in &workspaces {
        for file in BridgeStore::new(workspace.path.clone())?.list().await? {
            let bridge = file.bridge.context(format!(
                "Bridge {} : références illisibles",
                file.path.display()
            ))?;
            ensure!(
                !bridge.imports.values().any(|import| import.flow == key),
                "Suppression refusée : bridge {} dans {} référence ce flow",
                file.key,
                workspace.name
            );
        }
        validate_no_dependency(
            &flows.list(workspace).await?,
            &old.path,
            key,
            &mut package_preconditions,
        )
        .await?;
    }
    flow_packages::delete_package(PackageDeletion {
        workspace: if old.scope == "global" {
            home.into()
        } else {
            selected.path.clone()
        },
        target: old.path,
        expected_revision: expected_hash.into(),
        preconditions,
        package_preconditions: package_preconditions
            .into_iter()
            .map(|(path, revision)| PackagePrecondition { path, revision })
            .collect(),
    })
    .await
}

async fn participants(
    db: &SqlitePool,
    selected: &Workspace,
    home: &Path,
) -> Result<Vec<Workspace>> {
    let mut workspaces = workspaces::list(db).await?;
    workspaces.push(selected.clone());
    workspaces.push(Workspace {
        id: workspaces::path_id(home),
        name: "home".into(),
        path: home.into(),
        open: false,
    });
    workspaces.sort_by(|a, b| a.path.cmp(&b.path));
    workspaces.dedup_by(|a, b| a.path == b.path);
    for workspace in &workspaces {
        crate::live_files::recover(db, &workspace.path).await?;
    }
    Ok(workspaces)
}

async fn validate_no_dependency(
    files: &[FlowFile],
    target: &Path,
    excluded_key: &str,
    audited: &mut BTreeMap<PathBuf, String>,
) -> Result<()> {
    for file in files {
        let is_package_location = file.path.parent().is_some_and(|parent| {
            parent.file_name().is_some_and(|name| name == "flow")
                && parent
                    .parent()
                    .is_some_and(|root| root.file_name().is_some_and(|name| name == ".zedflow"))
        }) || file.path.ends_with(".zedflow/flow");
        if file.key == excluded_key || !is_package_location {
            continue;
        }
        if audited.contains_key(&file.path) {
            continue;
        }
        // Acquisition follows all declared dependency edges and returns exact
        // absolute preconditions, including transitive dependencies outside the
        // visible catalogue. Identity/hash equality alone would confuse copies.
        let captured = flow_packages::capture_with_preconditions(&file.path)
            .await
            .with_context(|| {
                format!(
                    "Fermeture de package invérifiable : {}",
                    file.path.display()
                )
            })?;
        ensure!(
            !captured
                .preconditions
                .iter()
                .any(|condition| condition.path.starts_with(target)),
            "Opération refusée : le package {} dépend de {}",
            file.path.display(),
            target.display()
        );
        audited.insert(file.path.clone(), captured.snapshot.root);
    }
    Ok(())
}

fn validate_remapped_resolutions(
    before: &CompositionCatalog,
    after: &CompositionCatalog,
    old_key: &str,
    new_key: &str,
) -> Result<()> {
    // Resolve each bridge with its complete `requires` closure, and the combined
    // selection when valid, so aliases/reuse and cross-bridge routes are checked.
    let mut selections: Vec<Vec<String>> =
        before.bridges.keys().map(|key| vec![key.clone()]).collect();
    selections.push(before.bridges.keys().cloned().collect());
    selections.push(vec![]);
    for bridges in selections {
        for (flow, contract) in &before.flows {
            for entry in contract.entries.keys() {
                let request = ResolveRequest {
                    flow: flow.clone(),
                    entry: entry.clone(),
                    bridges: bridges.clone(),
                };
                let Ok(mut previous) = resolve::resolve(before, &request) else {
                    continue;
                };
                for instance in previous.instances.values_mut() {
                    if instance.flow == old_key {
                        new_key.clone_into(&mut instance.flow);
                    }
                }
                for bridge in previous.bridges.values_mut() {
                    for import in bridge.imports.values_mut() {
                        if import.flow == old_key {
                            new_key.clone_into(&mut import.flow);
                        }
                    }
                }
                let next = resolve::resolve(
                    after,
                    &ResolveRequest {
                        flow: if flow == old_key {
                            new_key.into()
                        } else {
                            flow.clone()
                        },
                        ..request
                    },
                )
                .map_err(|errors| anyhow::anyhow!("Résolution invalidée : {errors:?}"))?;
                ensure!(
                    serde_json::to_value(previous)? == serde_json::to_value(next)?,
                    "La conversion modifierait la composition résolue"
                );
            }
        }
    }
    Ok(())
}
