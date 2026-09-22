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

/// Select a flow only after full interpretation. Identical captures share that
/// pure validation within this call, never across workspaces or authoring commands.
pub(crate) fn flow_definitions_from_snapshots(
    rows: &[RunDefinitionSnapshot],
    flow_key: &str,
) -> Result<Vec<RunDefinition>> {
    select_flow_definitions(rows, flow_key, definitions_from_snapshots)
}

fn select_flow_definitions(
    rows: &[RunDefinitionSnapshot],
    flow_key: &str,
    mut interpret: impl FnMut(&[RunDefinitionSnapshot]) -> Result<Vec<RunDefinition>>,
) -> Result<Vec<RunDefinition>> {
    let mut selected: Vec<RunDefinition> = Vec::new();
    let mut validated = BTreeMap::<Vec<u8>, std::ops::Range<usize>>::new();
    for row in rows {
        match validated.entry(definition_snapshot_key(row)?) {
            std::collections::btree_map::Entry::Occupied(entry) => {
                for index in entry.get().clone() {
                    let mut definition = selected[index].clone();
                    // The interpreter only copies this outer ID to its output;
                    // all embedded IDs and references belong to the exact key.
                    definition.run_id.clone_from(&row.run_id);
                    selected.push(definition);
                }
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                let start = selected.len();
                selected.extend(
                    interpret(std::slice::from_ref(row))?
                        .into_iter()
                        .filter(|run| run.definition.key == flow_key),
                );
                entry.insert(start..selected.len());
            }
        }
    }
    Ok(selected)
}

fn definition_snapshot_key(row: &RunDefinitionSnapshot) -> Result<Vec<u8>> {
    // Exhaustive patterns make additions to either storage type require a new
    // decision here. Compare full encodings, not hashes or purported identities.
    let RunDefinitionSnapshot {
        run_id: _,
        flow_ref,
        composition,
        flow_source,
        flow_package,
        runtime_graph,
        revision_heads,
        runtime_graph_head,
    } = row;
    fn head_parts(head: &snapshots::DefinitionHead) -> (&Value, &Value) {
        let snapshots::DefinitionHead { head, definition } = head;
        (head, definition)
    }
    let heads: BTreeMap<_, _> = revision_heads
        .iter()
        .map(|(key, head)| (key, head_parts(head)))
        .collect();
    Ok(serde_json::to_vec(&(
        flow_ref,
        composition,
        flow_source,
        flow_package,
        runtime_graph,
        heads,
        runtime_graph_head.as_ref().map(head_parts),
    ))?)
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

#[cfg(test)]
mod selection_tests {
    use super::*;
    use zf_storage::live_files::DefinitionHead;

    fn snapshot(id: &str) -> RunDefinitionSnapshot {
        let composition: Composition = serde_json::from_value(json!({
            "formatVersion":3,"id":"fixture","name":"Fixture",
            "nodes":[
                {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
                {"id":"e","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
            ],"edges":[{"id":"edge","source":"s","target":"e"}]
        })).unwrap();
        let source =
            zf_flows::flow_format::render(&composition, &GraphValidator::new(&RuntimePrimitives))
                .unwrap();
        RunDefinitionSnapshot {
            run_id: id.into(),
            flow_ref: json!({"key":"fixture-key"}),
            composition: serde_json::to_value(composition).unwrap(),
            flow_source: json!(source),
            flow_package: Value::Null,
            runtime_graph: Value::Null,
            revision_heads: BTreeMap::new(),
            runtime_graph_head: None,
        }
    }

    fn compare(rows: &[RunDefinitionSnapshot], key: &str) -> usize {
        let expected: Vec<_> = definitions_from_snapshots(rows)
            .unwrap()
            .into_iter()
            .filter(|run| run.definition.key == key)
            .collect();
        let mut validations = 0;
        let actual = select_flow_definitions(rows, key, |row| {
            let definitions = definitions_from_snapshots(row)?;
            validations += definitions.len();
            Ok(definitions)
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        validations
    }

    #[test]
    fn repeated_snapshots_validate_once_for_new_unused_and_affected_keys() {
        for count in [0, 8, 16] {
            let rows: Vec<_> = (0..count).map(|i| snapshot(&format!("run-{i}"))).collect();
            for key in ["new-key", "unused-existing-key", "fixture-key"] {
                assert_eq!(compare(&rows, key), usize::from(count > 0));
                // A separate invocation must validate again, not reuse a previous success.
                assert_eq!(compare(&rows, key), usize::from(count > 0));
            }
        }
    }

    #[test]
    fn distinct_snapshots_are_all_interpreted_even_when_unaffected() {
        let rows: Vec<_> = (0..8)
            .map(|i| {
                let mut row = snapshot(&format!("run-{i}"));
                let mut composition: Composition =
                    serde_json::from_value(row.composition.clone()).unwrap();
                composition.name = format!("Distinct history {i}");
                row.flow_source = json!(
                    zf_flows::flow_format::render(
                        &composition,
                        &GraphValidator::new(&RuntimePrimitives)
                    )
                    .unwrap()
                );
                row.composition = serde_json::to_value(composition).unwrap();
                row
            })
            .collect();
        assert_eq!(compare(&rows, "new-key"), 8);
    }

    #[test]
    fn every_snapshot_field_and_nested_identity_participates_in_the_exact_key() {
        fn complete(id: &str) -> RunDefinitionSnapshot {
            let mut row = snapshot(id);
            row.flow_package = json!({"closure":{"alias":"original"}});
            row.runtime_graph = json!({"flows":{"child":{"key":"child-key"}},"definitions":{"contextSelections":{"child/context":{"key":"policy"}}}});
            row.revision_heads.insert(
                "head".into(),
                DefinitionHead {
                    head: json!({"instance":"child","runId":"embedded-id"}),
                    definition: json!({"key":"child-key","source":"exact"}),
                },
            );
            row.runtime_graph_head = Some(DefinitionHead {
                head: json!({"graphRef":"adopted","runId":"embedded-id"}),
                definition: json!({"flows":{"child":{"key":"adopted-key"}},"aliases":{}}),
            });
            row
        }
        let key = definition_snapshot_key(&complete("first")).unwrap();
        assert_eq!(
            key,
            definition_snapshot_key(&complete("different-run")).unwrap()
        );
        for field in 0..13 {
            let mut row = complete("second");
            match field {
                0 => row.flow_ref["key"] = json!("alias"),
                1 => row.composition["name"] = json!("other"),
                2 => row.flow_source = json!(format!("{}\n", row.flow_source.as_str().unwrap())),
                3 => row.flow_package["closure"]["alias"] = json!("different"),
                4 => row.runtime_graph["flows"]["child"]["key"] = json!("alias"),
                5 => {
                    row.runtime_graph["definitions"]["contextSelections"]["child/context"]["key"] =
                        json!("other")
                }
                6 => {
                    let head = row.revision_heads.remove("head").unwrap();
                    row.revision_heads.insert("different-key".into(), head);
                }
                7 => row.revision_heads.get_mut("head").unwrap().head["instance"] = json!("root"),
                8 => {
                    row.revision_heads.get_mut("head").unwrap().definition["source"] =
                        json!("different bytes")
                }
                9 => {
                    row.runtime_graph_head.as_mut().unwrap().head["runId"] =
                        json!("different embedded id")
                }
                10 => {
                    row.runtime_graph_head.as_mut().unwrap().definition["aliases"] =
                        json!({"new":"alias"})
                }
                11 => row.runtime_graph_head = None,
                12 => row.revision_heads.clear(),
                _ => unreachable!(),
            }
            assert_ne!(key, definition_snapshot_key(&row).unwrap(), "field {field}");
            // Count misses independently of interpretation, including fields the
            // old interpreter does not inspect (e.g. adopted runtime graph heads).
            let mut calls = 0;
            select_flow_definitions(&[complete("first"), row], "unused", |_| {
                calls += 1;
                Ok(vec![])
            })
            .unwrap();
            assert_eq!(calls, 2, "field {field}");
        }
    }

    #[test]
    fn package_closure_aliases_and_bytes_are_not_authenticated_by_root_alone() {
        use zf_flows::package::PackageSnapshot;
        let leaf = snapshot("leaf");
        let leaf = PackageSnapshot::capture(
            json!({"formatVersion":1,"id":"dependency","name":"Dependency","entry":"flow.rs","files":["flow.rs"]}).to_string(),
            BTreeMap::from([("flow.rs".into(), leaf.flow_source.as_str().unwrap().replace("fixture", "dependency").into_bytes())]),
            BTreeMap::new(),
        ).unwrap();
        let packaged = |id: &str, alias: &str| {
            let mut row = snapshot(id);
            let package = PackageSnapshot::capture(
                json!({"formatVersion":1,"id":"fixture","name":"Fixture","entry":"flow.rs","files":["flow.rs"],"dependencies":{alias:{"path":"../dependency"}}}).to_string(),
                BTreeMap::from([("flow.rs".into(), row.flow_source.as_str().unwrap().as_bytes().to_vec())]),
                BTreeMap::from([(alias.into(), leaf.clone())]),
            ).unwrap();
            row.flow_package = serde_json::to_value(package).unwrap();
            row
        };
        let rows = [
            packaged("a", "first"),
            packaged("b", "first"),
            packaged("c", "second"),
        ];
        assert_eq!(compare(&rows, "fixture-key"), 2);
        assert_eq!(compare(&rows, "unused-key"), 2);
        let mut corrupt = packaged("bad", "first");
        corrupt.flow_package["packages"][&leaf.root]["files"]["flow.rs"] =
            json!("tampered dependency");
        let rows = [packaged("valid", "first"), corrupt];
        let expected = definitions_from_snapshots(&rows).unwrap_err().to_string();
        assert_eq!(
            flow_definitions_from_snapshots(&rows, "unused-key")
                .unwrap_err()
                .to_string(),
            expected
        );
    }

    #[tokio::test]
    async fn flow_selection_remains_under_the_start_and_authoring_lock() {
        use crate::{
            authoring::StoreFlow,
            commands::{Actor, CommandAuthorizer, CommandKind},
            service::{ExecutionOptions, ExecutionService},
            start::{StartDefinition, StartRequest},
        };
        use std::sync::Arc;
        struct Authority;
        #[async_trait::async_trait]
        impl CommandAuthorizer for Authority {
            async fn authorize(
                &self,
                _: &Actor,
                _: CommandKind,
                _: &zf_storage::workspaces::Workspace,
                _: Option<&Value>,
            ) -> Result<()> {
                Ok(())
            }
        }
        let root = tempfile::tempdir().unwrap();
        for name in ["workspace", "home"] {
            std::fs::create_dir(root.path().join(name)).unwrap();
        }
        let service = ExecutionService::open(ExecutionOptions {
            data: root.path().join("data"),
            workspace: root.path().join("workspace"),
            flow_home: root.path().join("home"),
            context_home: Some(root.path().join("home")),
            skill_dirs: vec![],
            authorizer: Arc::new(Authority),
        })
        .await
        .unwrap();
        let actor = Actor {
            id: "fixture".into(),
            workspace_id: service.default_workspace_id().into(),
        };
        let mut composition = snapshot("source").composition;
        composition["nodes"].as_array_mut().unwrap().push(json!({"id":"wait","position":{"x":0,"y":0},"data":{"kind":"input","label":"Wait","config":{"field":"answer","prompt":"Continue?","responseType":"text"}}}));
        composition["edges"] = json!([{"id":"a","source":"s","target":"wait"},{"id":"b","source":"wait","target":"e"}]);
        let file = service
            .store_flow(
                &actor,
                StoreFlow {
                    composition: serde_json::from_value(composition).unwrap(),
                    scope: None,
                    key: None,
                    expected_hash: None,
                },
            )
            .await
            .unwrap();
        let request = || StartRequest {
            definition: StartDefinition::Stored {
                key: file.key.clone(),
                expected_hash: file.hash.clone(),
            },
            input: Default::default(),
            model_bindings: json!({}),
            node_path: None,
            prepared_context: None,
            preview_metadata: None,
        };
        let first = service.start(&actor, request()).await.unwrap();
        service
            .wait_idle(first["id"].as_str().unwrap())
            .await
            .unwrap();
        let context = service
            .admit(&actor, CommandKind::Read, None)
            .await
            .unwrap();
        let guard = context.authoring_writer.lock().await;
        let initial = snapshots::snapshots(&service.database(), &actor.workspace_id)
            .await
            .unwrap();
        assert_eq!(initial.len(), 1);
        let mut changed = file.composition.clone().unwrap();
        changed.name = "Published under lock".into();
        let mut start = Box::pin(service.start(&actor, request()));
        let mut save = Box::pin(service.store_flow(
            &actor,
            StoreFlow {
                composition: changed,
                scope: None,
                key: Some(file.key.clone()),
                expected_hash: Some(file.hash.clone()),
            },
        ));
        // Poll both commands while holding the actual service lock. Neither a
        // new run nor a head can publish before the coherent capture is allowed.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut start)
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut save)
                .await
                .is_err()
        );
        let before = snapshots::snapshots(&service.database(), &actor.workspace_id)
            .await
            .unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(
            definition_snapshot_key(&before[0]).unwrap(),
            definition_snapshot_key(&initial[0]).unwrap()
        );
        drop(guard);
        let (started, saved) = tokio::join!(start, save);
        let started = started.unwrap();
        service
            .wait_idle(started["id"].as_str().unwrap())
            .await
            .unwrap();
        let saved = saved.unwrap();
        let definitions = run_definitions(&service.database(), &actor.workspace_id)
            .await
            .unwrap();
        assert_eq!(definitions.len(), 2);
        for definition in definitions {
            assert_eq!(
                definition.definition.source,
                saved.source.as_ref().unwrap().as_str()
            );
        }
        drop(context);
        service.shutdown().await.unwrap();
    }

    #[test]
    fn changed_source_and_malformed_heads_never_reuse_a_success() {
        for corruption in 0..5 {
            let good = snapshot("good");
            let mut bad = snapshot("bad");
            if corruption == 1 || corruption == 2 {
                let mut definition = serde_json::to_value(
                    &definitions_from_snapshots(std::slice::from_ref(&good)).unwrap()[0].definition,
                )
                .unwrap();
                if corruption == 2 {
                    definition["key"] = json!(42);
                }
                bad.revision_heads.insert(
                    zf_storage::context_store::hash(b""),
                    DefinitionHead {
                        head: json!({"instance":if corruption == 1 { "wrong" } else { "" }}),
                        definition,
                    },
                );
            } else if corruption == 3 {
                bad.runtime_graph = json!({"flows":{"child":{"key":42}}});
            } else if corruption == 4 {
                bad.composition["id"] = json!(42);
            } else {
                bad.flow_source = json!("invalid source");
            }
            let rows = [good, bad];
            let expected = definitions_from_snapshots(&rows).unwrap_err().to_string();
            let mut calls = 0;
            let error = select_flow_definitions(&rows, "new-key", |row| {
                calls += 1;
                definitions_from_snapshots(row)
            })
            .unwrap_err();
            assert_eq!(error.to_string(), expected);
            assert_eq!(calls, 2);
        }
        let mut rows = [snapshot("a"), snapshot("b")];
        for row in &mut rows {
            row.flow_source = Value::Null;
        }
        let mut calls = 0;
        assert!(
            select_flow_definitions(&rows, "unused", |row| {
                calls += 1;
                definitions_from_snapshots(row)
            })
            .is_err()
        );
        assert_eq!(calls, 1, "first error must stop interpretation");
    }
}
