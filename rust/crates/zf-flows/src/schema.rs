//! Persisted Vue Flow documents. Presentation is separate from executable data.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Composition {
    #[serde(default = "legacy_format", skip_serializing_if = "is_legacy_format")]
    pub format_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub revision: i64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub settings: GraphSettings,
    #[serde(default)]
    pub channels: Vec<StateChannel>,
}

fn legacy_format() -> u32 {
    1
}
fn is_legacy_format(version: &u32) -> bool {
    *version == 1
}

/// Persisted settings map to ADK's compiled graph policies, not a second executor.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphSettings {
    /// Explicit flow cwd, relative to the containing workspace (or parent subgraph).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    pub recursion_limit: usize,
    pub max_concurrency: Option<usize>,
    pub strict_channels: bool,
    pub timeout_ms: Option<u64>,
    pub idle_timeout_ms: Option<u64>,
    pub retry: Option<RetrySettings>,
}
impl Default for GraphSettings {
    fn default() -> Self {
        Self {
            working_directory: None,
            recursion_limit: 100,
            max_concurrency: None,
            strict_channels: false,
            timeout_ms: None,
            idle_timeout_ms: None,
            retry: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RetrySettings {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
    pub jitter: f64,
    pub retry_on: String,
}
impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            initial_delay_ms: 1000,
            max_delay_ms: 60_000,
            backoff_factor: 2.0,
            jitter: 0.0,
            retry_on: "any".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateChannel {
    pub name: String,
    #[serde(default = "overwrite")]
    pub reducer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}
fn overwrite() -> String {
    "overwrite".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    #[serde(rename = "type", default = "node_type")]
    pub node_type: String,
    pub position: Position,
    pub data: NodeData,
}
fn node_type() -> String {
    "flow".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeData {
    pub label: String,
    pub kind: String,
    #[serde(default)]
    pub config: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub source_handle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_handle: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

/// Standard shared-state channels available to every historical flow. Runtime
/// adapters materialize their ADK reducers from this same public definition.
pub const BUILTIN_CHANNELS: &[&str] = &[
    "input",
    "output",
    "response",
    "messages",
    "toolCalls",
    "toolResults",
    "hasToolCalls",
    "modelResponse",
    "hasSteering",
    "hasFollowUp",
    "__zedflow:consumedMessages",
    "__zedflow:context",
];

/// Channel names available to port contracts, including the standard channels.
/// Unknown reducers are rejected before an ADK graph can be materialized.
pub fn channel_names(
    channels: &[StateChannel],
) -> anyhow::Result<std::collections::BTreeSet<&str>> {
    let mut names: std::collections::BTreeSet<&str> = BUILTIN_CHANNELS.iter().copied().collect();
    for channel in channels {
        anyhow::ensure!(
            matches!(channel.reducer.as_str(), "overwrite" | "append" | "sum"),
            "Reducer inconnu : {}",
            channel.reducer
        );
        names.insert(&channel.name);
    }
    Ok(names)
}

pub fn runtime_channels(doc: &Composition) -> anyhow::Result<Value> {
    let mut channels = serde_json::to_value(&doc.channels)?;
    let list = channels
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("Canaux invalides"))?;
    // Responses to durable waits use one private overwrite channel per input node.
    for path in answer_paths(doc)? {
        list.push(serde_json::json!({"name":format!("answer:{path}"),"reducer":"overwrite"}));
    }
    for node in doc
        .nodes
        .iter()
        .filter(|node| ["input", "inbox", "steering"].contains(&node.data.kind.as_str()))
    {
        list.push(
            serde_json::json!({"name":format!("__zedflow:answerConsumed:{}",node.id),"reducer":"overwrite"}),
        );
        list.push(serde_json::json!({"name":format!("__zedflow:input:{}",node.data.config["field"].as_str().unwrap_or(if node.data.kind == "input" { "output" } else { "input" })),"reducer":"overwrite"}));
    }
    for node in doc
        .nodes
        .iter()
        .filter(|node| matches!(node.data.kind.as_str(), "agent" | "model"))
    {
        list.push(
            serde_json::json!({"name":format!("__zedflow:model-input:{}",node.id),"reducer":"overwrite"}),
        );
    }
    for node in doc.nodes.iter().filter(|node| node.data.kind == "model") {
        list.push(
            serde_json::json!({"name":format!("__zedflow:prepared-consumed:{}",node.id),"reducer":"overwrite"}),
        );
    }
    for node in doc
        .nodes
        .iter()
        .filter(|node| node.data.kind == "context" && doc.format_version >= 3)
    {
        list.push(serde_json::json!({"name":format!("__zedflow:prepared:{}",node.id),"reducer":"overwrite"}));
    }
    if doc.format_version >= 2 {
        for node in doc
            .nodes
            .iter()
            .filter(|node| node.data.kind == "condition")
        {
            list.push(
                serde_json::json!({"name":format!("__zedflow:condition:{}", node.id),"reducer":"overwrite"}),
            );
        }
    }
    Ok(channels)
}

pub fn answer_paths(doc: &Composition) -> anyhow::Result<Vec<String>> {
    let mut paths = Vec::new();
    for node in &doc.nodes {
        if ["input", "inbox"].contains(&node.data.kind.as_str()) {
            paths.push(node.id.clone());
        }
        if node.data.kind == "subgraph" {
            let child: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            paths.extend(
                answer_paths(&child)?
                    .into_iter()
                    .map(|path| format!("{}/{path}", node.id)),
            );
        }
    }
    Ok(paths)
}
