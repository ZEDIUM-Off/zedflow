//! Validated, serializable instructions for ADK materialization. No executor or
//! service handle belongs to this document; runtime adapters consume its getters.
use crate::graph_compiler::{self, PrimitiveContracts};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zf_flows::schema::{self, Composition, GraphSettings};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlannedEdge {
    Direct {
        source: String,
        target: String,
    },
    /// A route avoids ADK's all-predecessor join for an explicit fanIn:any.
    Alternative {
        source: String,
        target: String,
    },
    Conditional {
        source: String,
        field: String,
        expected: Value,
        yes: String,
        no: String,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    /// Effective config includes the node identity and historical semantics.
    pub config: Value,
    pub child: Option<Box<GraphPlan>>,
}

/// Constructed only by lowering a validated document. Deserializing an arbitrary
/// JSON object cannot bypass validation; persisted definitions are recompiled.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphPlan {
    format_version: u32,
    nodes: Vec<PlannedNode>,
    edges: Vec<PlannedEdge>,
    channels: Value,
    answer_paths: Vec<String>,
    settings: GraphSettings,
}
impl GraphPlan {
    pub fn format_version(&self) -> u32 {
        self.format_version
    }
    pub fn nodes(&self) -> &[PlannedNode] {
        &self.nodes
    }
    pub fn edges(&self) -> &[PlannedEdge] {
        &self.edges
    }
    pub fn channels(&self) -> &Value {
        &self.channels
    }
    pub fn answer_paths(&self) -> &[String] {
        &self.answer_paths
    }
    pub fn settings(&self) -> &GraphSettings {
        &self.settings
    }
    /// Includes executable settings, nested plans, configs and edge order.
    pub fn revision(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

pub fn lower(doc: &Composition, primitives: &dyn PrimitiveContracts) -> Result<GraphPlan> {
    graph_compiler::validate(doc, primitives)?;
    lower_validated(doc)
}

// Root validation has already traversed and bounded all children.
fn lower_validated(doc: &Composition) -> Result<GraphPlan> {
    let mut nodes = Vec::new();
    for node in &doc.nodes {
        if matches!(node.data.kind.as_str(), "start" | "end") {
            continue;
        }
        let child = if node.data.kind == "subgraph" {
            Some(Box::new(lower_validated(&serde_json::from_value(
                node.data.config["composition"].clone(),
            )?)?))
        } else {
            None
        };
        nodes.push(PlannedNode {
            id: node.id.clone(),
            label: node.data.label.clone(),
            kind: node.data.kind.clone(),
            config: graph_compiler::config(node, doc.format_version),
            child,
        });
    }
    let mut edges = Vec::new();
    for node in &doc.nodes {
        let outgoing: Vec<_> = doc
            .edges
            .iter()
            .filter(|edge| edge.source == node.id)
            .collect();
        if node.data.kind == "condition" {
            let yes = outgoing
                .iter()
                .find(|e| e.source_handle.as_deref() == Some("true"))
                .context("true absent")?;
            let no = outgoing
                .iter()
                .find(|e| e.source_handle.as_deref() == Some("false"))
                .context("false absent")?;
            edges.push(PlannedEdge::Conditional {
                source: node.id.clone(),
                field: if doc.format_version >= 2 {
                    format!("__zedflow:condition:{}", node.id)
                } else {
                    node.data.config["field"].as_str().unwrap_or("input").into()
                },
                expected: if doc.format_version >= 2 {
                    json!(true)
                } else {
                    node.data
                        .config
                        .get("equals")
                        .cloned()
                        .unwrap_or(json!(true))
                },
                yes: graph_compiler::target(doc, &yes.target),
                no: graph_compiler::target(doc, &no.target),
            });
        } else {
            for edge in outgoing {
                let target = graph_compiler::target(doc, &edge.target);
                edges.push(
                    if node.data.kind != "start"
                        && graph_compiler::alternative_arrival(doc, &edge.target)
                    {
                        PlannedEdge::Alternative {
                            source: node.id.clone(),
                            target,
                        }
                    } else {
                        PlannedEdge::Direct {
                            source: if node.data.kind == "start" {
                                graph_compiler::START.into()
                            } else {
                                node.id.clone()
                            },
                            target,
                        }
                    },
                );
            }
        }
    }
    Ok(GraphPlan {
        format_version: doc.format_version,
        nodes,
        edges,
        channels: schema::runtime_channels(doc)?,
        answer_paths: schema::answer_paths(doc)?,
        settings: doc.settings.clone(),
    })
}
