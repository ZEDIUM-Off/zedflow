//! Compile a captured catalogue, without reading files or constructing services.
pub use crate::prepared_model::{ContextSelection, DefinitionPins, FrozenFlow, PreparedRuntime};
use crate::{
    graph_compiler::{GraphValidator, PrimitiveContracts},
    programs::{self, ProgramSources, SourceSnapshot},
    resolve,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zf_flows::{
    bridge_source,
    composition::{CompositionCatalog, ResolveRequest},
    flow_contract, flow_format,
};

/// The capturing service checks filesystem/catalogue preconditions before
/// constructing this input and before admitting execution. No paths are read here.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompilationSnapshot {
    pub flows: BTreeMap<String, SourceSnapshot>,
    pub bridges: BTreeMap<String, SourceSnapshot>,
    pub programs: ProgramSources,
}

/// Structured resolution failure retained through anyhow for callers that also
/// need context-rich source/configuration errors.
#[derive(Debug)]
pub struct ResolutionFailure {
    pub diagnostics: Vec<zf_core::diagnostics::Diagnostic>,
    pub invalid_sources: BTreeMap<String, String>,
}
impl std::fmt::Display for ResolutionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Composition resolution failed: {:?}; invalid sources: {:?}",
            self.diagnostics, self.invalid_sources
        )
    }
}
impl std::error::Error for ResolutionFailure {}

pub fn prepare(
    snapshot: &CompilationSnapshot,
    request: &ResolveRequest,
    expected_hashes: &BTreeMap<String, String>,
    contexts: &BTreeMap<String, ContextSelection>,
    primitives: &dyn PrimitiveContracts,
) -> Result<PreparedRuntime> {
    let validator = GraphValidator::new(primitives);
    let mut catalog = CompositionCatalog::default();
    let mut definitions = BTreeMap::new();
    let mut invalid = BTreeMap::new();
    // Invalid unrelated catalogue entries remain diagnosable without preventing
    // a valid selection, matching historical discovery. Selected failures are
    // reported by resolution together with their source diagnostics.
    for (key, file) in &snapshot.flows {
        let parsed = file
            .validate()
            .and_then(|()| flow_format::parse(&file.source, &validator));
        match parsed {
            Ok(doc) => {
                if let Some(exports) = flow_contract::validate(&doc)? {
                    for (name, ty) in &exports.types {
                        if let Some(previous) = catalog.types.insert(name.clone(), ty.clone()) {
                            ensure!(previous == *ty, "Conflicting shared type {name}");
                        }
                    }
                    catalog.flows.insert(key.clone(), exports.contract);
                    definitions.insert(key.clone(), doc);
                }
            }
            Err(error) => {
                invalid.insert(format!("flows.{key}"), format!("{error:#}"));
            }
        }
    }
    for (key, file) in &snapshot.bridges {
        let parsed = file.validate().and_then(|()| {
            bridge_source::parse(&file.source).map_err(|d| anyhow::anyhow!("{d:?}"))
        });
        match parsed {
            Ok(bridge) => {
                catalog.bridges.insert(key.clone(), bridge);
            }
            Err(error) => {
                invalid.insert(format!("bridges.{key}"), format!("{error:#}"));
            }
        }
    }
    let graph = resolve::resolve(&catalog, request).map_err(|diagnostics| ResolutionFailure {
        diagnostics,
        invalid_sources: invalid,
    })?;
    let mut selections: BTreeMap<String, BTreeMap<String, ContextSelection>> = BTreeMap::new();
    for (path, selected) in contexts {
        let instance = graph
            .instances
            .keys()
            .filter(|instance| path.starts_with(&format!("{instance}/")))
            .max_by_key(|instance| instance.len())
            .with_context(|| format!("Context selection {path}: unknown flow instance"))?;
        let source = snapshot
            .programs
            .strategies
            .get(&selected.key)
            .with_context(|| format!("Context selection {path}: strategy absent"))?;
        source.validate()?;
        ensure!(
            source.hash == selected.hash,
            "Context strategy changed since runtime selection"
        );
        let strategy = zf_context::context_source::parse(&source.source)
            .map_err(|d| anyhow::anyhow!("{d:?}"))?;
        ensure!(
            strategy.id == selected.key,
            "Context strategy identity differs from selected key"
        );
        selections
            .entry(instance.clone())
            .or_default()
            .insert(path[instance.len() + 1..].into(), selected.clone());
    }
    let mut flows = BTreeMap::new();
    let mut pins = DefinitionPins {
        context_selections: contexts.clone(),
        ..Default::default()
    };
    for (instance, resolved) in &graph.instances {
        let file = &snapshot.flows[&resolved.flow];
        if let Some(expected) = expected_hashes.get(&resolved.flow) {
            ensure!(
                *expected == file.hash,
                "Flow changed since runtime graph selection"
            );
        }
        pins.flow_hashes
            .insert(resolved.flow.clone(), file.hash.clone());
        let mut composition = definitions
            .get(&resolved.flow)
            .context("Resolved source is absent")?
            .clone();
        let empty = BTreeMap::new();
        let selection = selections.get(instance).unwrap_or(&empty);
        apply_context_selections(&mut composition, selection, None)?;
        let dependencies = programs::freeze(&mut composition, &snapshot.programs)?;
        for (path, selected) in selection {
            ensure!(
                agent_config(&composition, path)?["contextProgram"]["hash"] == selected.hash,
                "Context strategy changed while preparing runtime"
            );
        }
        crate::graph_compiler::validate(&composition, primitives)?;
        let source = if dependencies.is_empty() && selection.is_empty() {
            file.source.clone()
        } else {
            flow_format::render(&composition, &validator)?
        };
        let exports =
            flow_contract::validate(&composition)?.context("Flow has no public exports")?;
        flows.insert(
            instance.clone(),
            FrozenFlow {
                key: resolved.flow.clone(),
                hash: programs::hash(source.as_bytes()),
                source,
                composition,
                exports,
            },
        );
    }
    for key in graph.bridges.keys() {
        let file = &snapshot.bridges[key];
        pins.bridge_hashes.insert(key.clone(), file.hash.clone());
        pins.bridge_sources.insert(key.clone(), file.source.clone());
    }
    let prepared = PreparedRuntime {
        graph,
        flows,
        definitions: pins,
    };
    prepared.validate(primitives)?;
    Ok(prepared)
}

fn agent_config(doc: &zf_flows::schema::Composition, path: &str) -> Result<serde_json::Value> {
    let (id, rest) = path
        .split_once('/')
        .map_or((path, None), |(id, rest)| (id, Some(rest)));
    let node = doc
        .nodes
        .iter()
        .find(|node| node.id == id)
        .with_context(|| format!("Context selection {path}: unknown node"))?;
    if let Some(rest) = rest {
        ensure!(
            node.data.kind == "subgraph",
            "Context selection {path}: path is not a subgraph"
        );
        agent_config(
            &serde_json::from_value(node.data.config["composition"].clone())?,
            rest,
        )
    } else {
        if node.data.kind == "model" {
            let context = node.data.config["contextNode"]
                .as_str()
                .context("Context node is absent")?;
            let context = doc
                .nodes
                .iter()
                .find(|n| n.id == context)
                .context("Context node is absent")?;
            return Ok(context.data.config.clone());
        }
        ensure!(
            node.data.kind == "agent",
            "Context selection {path}: node is not an inference"
        );
        Ok(node.data.config.clone())
    }
}
fn replace_context(
    doc: &mut zf_flows::schema::Composition,
    path: &str,
    selection: serde_json::Value,
) -> Result<()> {
    let (id, rest) = path
        .split_once('/')
        .map_or((path, None), |(id, rest)| (id, Some(rest)));
    let target_id = doc
        .nodes
        .iter()
        .find(|node| node.id == id)
        .and_then(|node| {
            (rest.is_none() && node.data.kind == "model")
                .then(|| node.data.config["contextNode"].as_str().map(str::to_owned))
                .flatten()
        })
        .unwrap_or_else(|| id.to_owned());
    let node = doc
        .nodes
        .iter_mut()
        .find(|node| node.id == target_id)
        .with_context(|| format!("Context selection {path}: unknown node"))?;
    if let Some(rest) = rest {
        ensure!(
            node.data.kind == "subgraph",
            "Context selection {path}: path is not a subgraph"
        );
        let mut child = serde_json::from_value(node.data.config["composition"].clone())?;
        replace_context(&mut child, rest, selection)?;
        node.data.config["composition"] = serde_json::to_value(child)?;
    } else {
        ensure!(
            matches!(node.data.kind.as_str(), "agent" | "context"),
            "Context selection {path}: node is not an inference"
        );
        node.data.config["contextStrategy"] = selection;
        if let Some(config) = node.data.config.as_object_mut() {
            config.remove("contextProgram");
        }
    }
    Ok(())
}
/// Preserve explicit per-instance choices when the underlying authored flow is
/// saved. Its latest accepted strategy pin follows normal source lineage; the
/// initial selection hash remains in the immutable runtime metadata.
pub fn apply_context_selections(
    doc: &mut zf_flows::schema::Composition,
    selections: &BTreeMap<String, ContextSelection>,
    previous: Option<&zf_flows::schema::Composition>,
) -> Result<()> {
    for (path, selection) in selections {
        let reference = if let Some(previous) = previous {
            let config = agent_config(previous, path)?;
            let reference = &config["contextStrategy"];
            ensure!(
                reference.as_str().or(reference["key"].as_str()) == Some(selection.key.as_str()),
                "Context selection {path}: previous strategy differs from the runtime choice"
            );
            reference.clone()
        } else {
            serde_json::to_value(selection)?
        };
        replace_context(doc, path, reference)?;
    }
    Ok(())
}
