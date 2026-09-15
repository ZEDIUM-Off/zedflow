//! Named control boundaries describe ADK shared state without pretending that
//! one incoming edge owns every channel at a join.
use crate::schema::{Composition, Node};
use anyhow::{Result, ensure};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zf_core::diagnostics::Diagnostic;

// Predicate syntax belongs to the node contract; evaluating it against a
// state belongs to the runtime. Keep one parser for authoring and compilation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Predicate {
    All {
        items: Vec<Predicate>,
    },
    Any {
        items: Vec<Predicate>,
    },
    Compare {
        field: String,
        operator: PredicateOperator,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        value: Option<Value>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PredicateOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    Exists,
    Contains,
    In,
}

fn present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

pub fn parse_predicate(value: &Value) -> Result<Predicate> {
    let predicate: Predicate = serde_json::from_value(value.clone())?;
    validate_predicate(&predicate, 0)?;
    Ok(predicate)
}

fn validate_predicate(predicate: &Predicate, depth: usize) -> Result<()> {
    ensure!(depth <= 16, "Conditions limitées à 16 niveaux");
    match predicate {
        Predicate::All { items } | Predicate::Any { items } => {
            ensure!(
                !items.is_empty() && items.len() <= 64,
                "Un groupe ET/OU doit contenir 1 à 64 conditions"
            );
            for item in items {
                validate_predicate(item, depth + 1)?;
            }
        }
        Predicate::Compare {
            field,
            operator,
            value,
        } => {
            ensure!(
                !field.is_empty() && field.len() <= 1024,
                "Champ de condition invalide"
            );
            if field.starts_with('/') {
                let chars: Vec<_> = field.chars().collect();
                for (index, ch) in chars.iter().enumerate() {
                    ensure!(
                        *ch != '~' || matches!(chars.get(index + 1), Some('0' | '1')),
                        "JSON Pointer invalide : {field}"
                    );
                }
            }
            if matches!(operator, PredicateOperator::Exists) {
                ensure!(value.is_none(), "exists ne prend pas de valeur");
            } else {
                let value = value
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Valeur de comparaison requise"))?;
                if matches!(
                    operator,
                    PredicateOperator::Gt
                        | PredicateOperator::Gte
                        | PredicateOperator::Lt
                        | PredicateOperator::Lte
                ) {
                    ensure!(
                        value.is_number(),
                        "Une comparaison numérique requiert un nombre"
                    );
                }
                if matches!(operator, PredicateOperator::In) {
                    ensure!(
                        value.is_array() || value.is_string(),
                        "in attend un tableau ou une chaîne"
                    );
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BoundaryType {
    State,
    PreparedContext,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Port {
    pub id: String,
    pub label: String,
    pub data_type: BoundaryType,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeContract {
    pub node_id: String,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeAnalysis {
    pub edge_id: String,
    pub guaranteed: Vec<String>,
    pub conditional: Vec<String>,
    pub consumes: Vec<String>,
    pub produces: Vec<String>,
    pub unknown: bool,
    pub junction: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct GraphAnalysis {
    pub nodes: Vec<NodeContract>,
    pub edges: Vec<EdgeAnalysis>,
    pub diagnostics: Vec<Diagnostic>,
}
fn port(id: &str, label: &str, data_type: BoundaryType) -> Port {
    Port {
        id: id.into(),
        label: label.into(),
        data_type,
    }
}
fn template_fields(text: &str) -> Vec<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("{{") {
        let Some((name, after)) = after.split_once("}}") else {
            break;
        };
        names.insert(name.to_owned());
        rest = after;
    }
    names.into_iter().collect()
}
fn predicate_fields(value: &serde_json::Value) -> Vec<String> {
    let Ok(predicate) = parse_predicate(value) else {
        return vec![];
    };
    let mut pending = vec![&predicate];
    let mut names = BTreeSet::new();
    while let Some(item) = pending.pop() {
        match item {
            Predicate::All { items } | Predicate::Any { items } => pending.extend(items),
            Predicate::Compare { field, .. } => {
                let name = field
                    .strip_prefix('/')
                    .map(|value| {
                        value
                            .split('/')
                            .next()
                            .unwrap_or_default()
                            .replace("~1", "/")
                            .replace("~0", "~")
                    })
                    .unwrap_or_else(|| field.clone());
                names.insert(name);
            }
        }
    }
    names.into_iter().collect()
}
pub fn contract(node: &Node) -> NodeContract {
    let kind = node.data.kind.as_str();
    let config = &node.data.config;
    let field = |key: &str, default: &str| config[key].as_str().unwrap_or(default).to_owned();
    let inputs = match kind {
        "start" => vec![],
        "model" => vec![port(
            "context",
            "Contexte préparé",
            BoundaryType::PreparedContext,
        )],
        _ => vec![port("state", "État disponible", BoundaryType::State)],
    };
    let outputs = match kind {
        "end" => vec![],
        "context" => vec![port(
            "context",
            "Contexte préparé",
            BoundaryType::PreparedContext,
        )],
        "condition" => vec![
            port("true", "Oui", BoundaryType::State),
            port("false", "Non", BoundaryType::State),
        ],
        _ => vec![port("state", "État produit", BoundaryType::State)],
    };
    let consumes = match kind {
        "output" => template_fields(config["text"].as_str().unwrap_or("{{output}}")),
        "condition" if config["predicate"].is_object() => predicate_fields(&config["predicate"]),
        "condition" => vec![field("field", "output")],
        "set" => config["value"]
            .as_str()
            .map(template_fields)
            .unwrap_or_default(),
        "model" => vec![],
        "tool" | "route" | "await_route" => config["inputField"]
            .as_str()
            .map(|v| vec![v.into()])
            .unwrap_or_default(),
        _ => vec![],
    };
    let produces = match kind {
        "output" => vec!["response".into()],
        "set" | "tool" | "route" | "await_route" => vec![field("field", "output")],
        "input" | "inbox" => vec![field("field", "input")],
        "model" | "agent" => vec![
            field("field", "output"),
            field("historyField", "messages"),
            field("toolCallsField", "toolCalls"),
            "hasToolCalls".into(),
            "modelResponse".into(),
        ],
        _ => vec![],
    };
    NodeContract {
        node_id: node.id.clone(),
        inputs,
        outputs,
        consumes,
        produces,
    }
}

pub fn analyze(doc: &Composition) -> GraphAnalysis {
    let nodes: Vec<_> = doc.nodes.iter().map(contract).collect();
    let by_id: BTreeMap<_, _> = nodes.iter().map(|n| (n.node_id.as_str(), n)).collect();
    let initial: BTreeSet<String> = doc
        .channels
        .iter()
        .filter(|c| c.default.is_some())
        .map(|c| c.name.clone())
        .collect();
    let mut possible: BTreeMap<String, BTreeSet<String>> = nodes
        .iter()
        .map(|n| (n.node_id.clone(), initial.clone()))
        .collect();
    // Least fixed point for possible availability. Guaranteed channels remain
    // conservative around cycles; unknown runtime input is never asserted.
    for _ in 0..=nodes.len() {
        let previous = possible.clone();
        for node in &nodes {
            let values = possible.get_mut(&node.node_id).expect("known node");
            for edge in doc.edges.iter().filter(|e| e.target == node.node_id) {
                if let Some(source) = previous.get(&edge.source) {
                    values.extend(source.iter().cloned());
                }
                if let Some(source) = by_id.get(edge.source.as_str()) {
                    values.extend(source.produces.iter().cloned());
                }
            }
        }
        if previous == possible {
            break;
        }
    }
    let mut diagnostics = vec![];
    let mut edges = vec![];
    for edge in &doc.edges {
        let (Some(source), Some(target)) = (
            by_id.get(edge.source.as_str()),
            by_id.get(edge.target.as_str()),
        ) else {
            continue;
        };
        let output = if let Some(handle) = edge.source_handle.as_deref() {
            source.outputs.iter().find(|p| p.id == handle)
        } else {
            source.outputs.first()
        };
        let input = if let Some(handle) = edge.target_handle.as_deref() {
            target.inputs.iter().find(|p| p.id == handle)
        } else {
            target.inputs.first()
        };
        match (output, input) {
            (Some(output), Some(input)) if output.data_type == input.data_type => {}
            (Some(output), Some(input)) => diagnostics.push(Diagnostic::new(
                "port_type",
                format!("edges.{}", edge.id),
                format!(
                    "{} → {} : attendu {:?}, obtenu {:?}",
                    source.node_id, target.node_id, input.data_type, output.data_type
                ),
            )),
            _ => diagnostics.push(Diagnostic::new(
                "unknown_port",
                format!("edges.{}", edge.id),
                "Port d’entrée ou de sortie absent",
            )),
        }
        let incoming: Vec<_> = doc
            .edges
            .iter()
            .filter(|e| e.target == edge.target)
            .collect();
        let mut guaranteed = initial.clone();
        if incoming.len() == 1
            && doc.nodes.iter().any(|node| {
                node.id == edge.source
                    && matches!(node.data.kind.as_str(), "set" | "input" | "model" | "agent")
            })
        {
            guaranteed.extend(source.produces.iter().cloned());
        }
        let available = possible.get(&edge.target).cloned().unwrap_or_default();
        let conditional = available.difference(&guaranteed).cloned().collect();
        edges.push(EdgeAnalysis {
            edge_id: edge.id.clone(),
            guaranteed: guaranteed.into_iter().collect(),
            conditional,
            consumes: target.consumes.clone(),
            produces: source.produces.clone(),
            unknown: true,
            junction: incoming.len() > 1,
        });
    }
    GraphAnalysis {
        nodes,
        edges,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn flow() -> Composition {
        serde_json::from_value(json!({"id":"test","name":"Test","revision":0,"formatVersion":4,"nodes":[{"id":"a","position":{"x":0,"y":0},"data":{"kind":"context","label":"Contexte","config":{}}},{"id":"b","position":{"x":200,"y":0},"data":{"kind":"model","label":"Modèle","config":{}}}],"edges":[{"id":"e","source":"a","target":"b","sourceHandle":"context","targetHandle":"context"}]})).unwrap()
    }
    #[test]
    fn validates_named_boundaries_without_equating_control_with_context() {
        let mut flow = flow();
        assert!(analyze(&flow).diagnostics.is_empty());
        flow.nodes[0].data.kind = "tool".into();
        flow.edges[0].source_handle = Some("state".into());
        assert_eq!(analyze(&flow).diagnostics[0].code, "port_type");
        flow.edges[0].target_handle = Some("missing".into());
        assert_eq!(analyze(&flow).diagnostics[0].code, "unknown_port");
    }

    #[test]
    fn consumed_channels_follow_templates_and_nested_predicate_paths() {
        let mut doc = flow();
        doc.nodes[0].data.kind = "output".into();
        doc.nodes[0].data.config = json!({"text":"{{answer}} / {{source}} / {{answer}}"});
        let output = contract(&doc.nodes[0]);
        assert_eq!(output.consumes, vec!["answer", "source"]);
        assert_eq!(output.produces, vec!["response"]);
        doc.nodes[0].data.kind = "condition".into();
        doc.nodes[0].data.config = json!({"predicate":{"kind":"all","items":[
            {"kind":"compare","field":"/result/status","operator":"eq","value":"ok"},
            {"kind":"any","items":[
                {"kind":"compare","field":"/files~1docs/count","operator":"gt","value":0},
                {"kind":"compare","field":"ready","operator":"exists"}
            ]}
        ]}});
        assert_eq!(
            contract(&doc.nodes[0]).consumes,
            vec!["files/docs", "ready", "result"]
        );
    }
}
