//! Public contracts belong to a flow source; bindings identify actual ADK nodes
//! and channels. Context projections remain in the separately selected strategy.
use crate::composition::{FlowDefinition, InvocationKind};
use crate::schema::Composition;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zf_core::types::{TypeRegistry, validate_value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryBinding {
    pub node: String,
    pub input_field: String,
    #[serde(default)]
    pub output_field: Option<String>,
}

/// Serialized in the start node's `exports` configuration, hence carried by the
/// exact structured Rust source and every existing flow import/export path.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowExports {
    pub contract: FlowDefinition,
    #[serde(default)]
    pub types: TypeRegistry,
    pub entries: BTreeMap<String, EntryBinding>,
    #[serde(default)]
    pub branches: BTreeMap<String, String>,
    #[serde(default)]
    pub data: BTreeMap<String, String>,
    #[serde(default)]
    pub requires: BTreeMap<String, String>,
    #[serde(default)]
    pub interactive: bool,
}

pub fn read(doc: &Composition) -> Result<Option<FlowExports>> {
    let start = doc
        .nodes
        .iter()
        .find(|n| n.data.kind == "start")
        .context("Flow start is absent")?;
    start
        .data
        .config
        .get("exports")
        .map(|value| serde_json::from_value(value.clone()).context("Invalid public flow exports"))
        .transpose()
}

pub fn can_request(exports: &FlowExports, branch: &str, node: &str) -> bool {
    let Some(point) = exports.contract.branches.get(branch) else {
        return false;
    };
    if point.requesters.is_empty() {
        exports
            .branches
            .get(branch)
            .is_some_and(|owner| owner == node)
    } else {
        point.requesters.contains(node)
    }
}

pub fn requester_accepts(doc: &Composition, node: &str, invocation: InvocationKind) -> bool {
    doc.nodes
        .iter()
        .find(|item| item.id == node)
        .is_some_and(|item| match invocation {
            InvocationKind::Tool => matches!(item.data.kind.as_str(), "agent" | "model"),
            InvocationKind::Context => matches!(item.data.kind.as_str(), "agent" | "context"),
            InvocationKind::Node | InvocationKind::Condition => item.data.kind == "route",
        })
}

pub fn requesters<'a>(exports: &'a FlowExports, branch: &str) -> Vec<&'a str> {
    let Some(point) = exports.contract.branches.get(branch) else {
        return vec![];
    };
    if point.requesters.is_empty() {
        exports
            .branches
            .get(branch)
            .map(|node| vec![node.as_str()])
            .unwrap_or_default()
    } else {
        point.requesters.iter().map(String::as_str).collect()
    }
}

pub fn validate(doc: &Composition) -> Result<Option<FlowExports>> {
    let Some(exports) = read(doc)? else {
        return Ok(None);
    };
    let nodes: BTreeMap<_, _> = doc.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let fields = crate::schema::channel_names(&doc.channels)?;
    let field = |name: &str| -> Result<()> {
        ensure!(
            fields.contains(name),
            "Public contract references undeclared channel {name}"
        );
        Ok(())
    };
    ensure!(
        exports.entries.keys().eq(exports.contract.entries.keys()),
        "Each public entry needs exactly one node binding"
    );
    ensure!(
        exports.branches.keys().eq(exports.contract.branches.keys()),
        "Each branch point needs exactly one node binding"
    );
    ensure!(
        exports.data.keys().eq(exports.contract.data.keys()),
        "Each exposed dataset needs exactly one channel binding"
    );
    ensure!(
        exports.requires.keys().eq(exports.contract.requires.keys()),
        "Each required dataset needs exactly one channel binding"
    );
    for (name, binding) in &exports.entries {
        let node = nodes
            .get(binding.node.as_str())
            .with_context(|| format!("Entry {name}: node {} is absent", binding.node))?;
        ensure!(
            node.data.kind != "end",
            "Entry {name} cannot target an end node"
        );
        ensure!(
            node.data.kind != "model",
            "Entry {name} must enter through the model’s Context node, not bypass its preparation"
        );
        field(&binding.input_field)?;
        ensure!(
            binding.output_field.is_some() == exports.contract.entries[name].output.is_some(),
            "Entry {name}: output contract and channel must both be declared"
        );
        if let Some(output) = &binding.output_field {
            field(output)?;
        }
    }
    for (name, id) in &exports.branches {
        let node = nodes
            .get(id.as_str())
            .with_context(|| format!("Branch {name}: node {id} is absent"))?;
        let kinds = &exports.contract.branches[name].invocations;
        let requesters = &exports.contract.branches[name].requesters;
        ensure!(
            !kinds.is_empty() || !requesters.is_empty(),
            "Branch {name} must declare its authorized requesters or a legacy invocation contract"
        );
        if !requesters.is_empty() {
            ensure!(
                doc.format_version >= 4,
                "Neutral plug {name} requires Flow Rust v4"
            );
            ensure!(
                kinds.is_empty(),
                "Neutral plug {name} cannot declare a trigger policy"
            );
            ensure!(
                matches!(node.data.kind.as_str(), "route" | "context" | "model"),
                "Plug {name} must belong to a routing, context or model node"
            );
            for requester in requesters {
                let caller = nodes
                    .get(requester.as_str())
                    .with_context(|| format!("Plug {name}: requester {requester} is absent"))?;
                ensure!(
                    matches!(caller.data.kind.as_str(), "route" | "context" | "model"),
                    "Plug {name}: node {requester} cannot request a route"
                );
                if caller.data.kind == "route" {
                    ensure!(
                        caller.data.config["branch"] == *name,
                        "Plug {name}: requester {requester} selects another point"
                    );
                }
            }
            continue;
        }
        if kinds.contains(&InvocationKind::Tool) {
            ensure!(
                matches!(node.data.kind.as_str(), "agent" | "model"),
                "Tool branch {name} must be attached to an inference node"
            );
        }
        if kinds.contains(&InvocationKind::Context) {
            ensure!(
                matches!(node.data.kind.as_str(), "agent" | "context"),
                "Context branch {name} must be attached to an inference node"
            );
            let config = &node.data.config;
            let bindings = config
                .get("contextBindings")
                .or_else(|| config["contextProgram"].get("bindings"));
            let producer = bindings.and_then(Value::as_object).is_some_and(|bindings| {
                bindings.values().any(|binding| {
                    binding["kind"] == "produced" && binding["producer"]["branch"] == *name
                })
            });
            let window = config["contextProgram"]["window"]["prepare"]["branch"] == *name
                || config["contextWindow"]["prepare"]["branch"] == *name;
            ensure!(
                producer || window,
                "Context branch {name} is not used by a declared producer or window preparation"
            );
        }
        if kinds.contains(&InvocationKind::Node) || kinds.contains(&InvocationKind::Condition) {
            ensure!(
                node.data.kind == "route",
                "Branch {name} must be bound to a route node"
            );
            ensure!(
                node.data.config["branch"] == *name,
                "Branch {name} must match the route node’s selected public point"
            );
            if let Some(fallback) = node.data.config.get("fallback") {
                let output = exports.contract.branches[name]
                    .contract
                    .output
                    .as_ref()
                    .with_context(|| {
                        format!("Branch {name}: a fallback requires an output contract")
                    })?;
                validate_value(output, fallback, &exports.types)
                    .map_err(|d| anyhow::anyhow!("Branch {name} fallback: {d:?}"))?;
            }
        }
    }
    for (name, channel) in &exports.data {
        field(channel)?;
        if let Some(default) = doc
            .channels
            .iter()
            .find(|c| c.name == *channel)
            .and_then(|c| c.default.as_ref())
        {
            validate_value(
                &exports.contract.data[name].data_type,
                default,
                &exports.types,
            )
            .map_err(|d| anyhow::anyhow!("Dataset {name}: {d:?}"))?;
        }
    }
    for channel in exports.requires.values() {
        field(channel)?;
    }
    for (id, inference) in &exports.contract.inference_nodes {
        ensure!(
            nodes
                .get(id.as_str())
                .is_some_and(|n| matches!(n.data.kind.as_str(), "agent" | "model")),
            "Inference contract {id} has no model node"
        );
        for resource in &inference.resources {
            ensure!(
                exports.contract.data.contains_key(resource)
                    || exports.contract.requires.contains_key(resource),
                "Inference {id}: resource {resource} is not declared"
            );
        }
    }
    ensure!(
        exports.interactive
            || !doc
                .nodes
                .iter()
                .any(|n| matches!(n.data.kind.as_str(), "input" | "inbox")),
        "An autonomous flow cannot expose an interactive input node"
    );
    Ok(Some(exports))
}

/// Select a public entry without changing the authored positions or source.
/// Unreachable nodes are omitted only from this execution projection.
pub fn at_entry(doc: &Composition, entry: &str) -> Result<Composition> {
    let exports = validate(doc)?.context("Flow does not declare public exports")?;
    let binding = exports
        .entries
        .get(entry)
        .with_context(|| format!("Unknown flow entry {entry}"))?;
    let start = doc
        .nodes
        .iter()
        .find(|n| n.data.kind == "start")
        .context("Flow start is absent")?;
    let mut projected = doc.clone();
    if binding.node != start.id {
        projected.edges.retain(|edge| edge.source != start.id);
        projected.edges.push(crate::schema::Edge {
            id: format!("__entry_{entry}"),
            source: start.id.clone(),
            target: binding.node.clone(),
            source_handle: None,
            target_handle: None,
            label: None,
        });
    }
    let mut reachable = BTreeSet::from([start.id.clone()]);
    loop {
        let size = reachable.len();
        for edge in &projected.edges {
            if reachable.contains(&edge.source) {
                reachable.insert(edge.target.clone());
            }
        }
        if size == reachable.len() {
            break;
        }
    }
    projected.nodes.retain(|n| reachable.contains(&n.id));
    projected
        .edges
        .retain(|e| reachable.contains(&e.source) && reachable.contains(&e.target));
    // The projection has already been linked. Other public entries can become
    // unreachable; their authoring contract stays in the frozen original source.
    for node in &mut projected.nodes {
        if node.data.kind == "start"
            && let Some(config) = node.data.config.as_object_mut()
        {
            config.remove("exports");
        }
    }
    Ok(projected)
}

pub fn entry_input(
    exports: &FlowExports,
    entry: &str,
    input: Value,
) -> Result<std::collections::HashMap<String, Value>> {
    let contract = exports
        .contract
        .entries
        .get(entry)
        .context("Unknown entry")?;
    validate_value(&contract.input, &input, &exports.types)
        .map_err(|d| anyhow::anyhow!("Entry {entry}: {d:?}"))?;
    let binding = exports.entries.get(entry).context("Unbound entry")?;
    Ok([(binding.input_field.clone(), input)].into_iter().collect())
}
