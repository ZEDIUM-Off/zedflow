// @zedflow format v3. See docs/flow-format.md.
#![allow(dead_code, unused_imports)]
use adk_graph::prelude::*;
use adk_graph::edge::{Edge, EdgeTarget};
use std::{collections::HashMap, sync::Arc};
use zf_runtime::{models, operations, runtime, subgraphs};

const ZEDFLOW_FORMAT: u32 = 3;
// @zedflow flow-view: identity, name, revision
const FLOW: (&str, &str, i64) = ("working-system-harness", "Harness · Working System", 1);
// @zedflow node-view 0: identity, renderer, label, x, y
const NODE_0: (&str, &str, &str, f64, f64) = ("start", "flow", "Début", 0.0, 140.0);
// @zedflow node-view 1: identity, renderer, label, x, y
const NODE_1: (&str, &str, &str, f64, f64) = ("documentation", "flow", "Consulter Working System", 220.0, 120.0);
// @zedflow node-view 2: identity, renderer, label, x, y
const NODE_2: (&str, &str, &str, f64, f64) = ("context", "flow", "Préparer la restitution", 530.0, 120.0);
// @zedflow node-view 3: identity, renderer, label, x, y
const NODE_3: (&str, &str, &str, f64, f64) = ("model", "flow", "Modèle de restitution", 820.0, 120.0);
// @zedflow node-view 4: identity, renderer, label, x, y
const NODE_4: (&str, &str, &str, f64, f64) = ("response", "flow", "Réponse", 1110.0, 130.0);
// @zedflow node-view 5: identity, renderer, label, x, y
const NODE_5: (&str, &str, &str, f64, f64) = ("inbox", "flow", "Suite du travail", 1110.0, 380.0);
// @zedflow edge-view 0: identity, source handle, label
const EDGE_0: (&str, Option<&str>, Option<&str>) = ("start-documentation", None, None);
// @zedflow edge-view 1: identity, source handle, label
const EDGE_1: (&str, Option<&str>, Option<&str>) = ("documentation-context", None, None);
// @zedflow edge-view 2: identity, source handle, label
const EDGE_2: (&str, Option<&str>, Option<&str>) = ("context-model", None, None);
// @zedflow edge-view 3: identity, source handle, label
const EDGE_3: (&str, Option<&str>, Option<&str>) = ("model-response", None, None);
// @zedflow edge-view 4: identity, source handle, label
const EDGE_4: (&str, Option<&str>, Option<&str>) = ("response-inbox", None, None);
// @zedflow edge-view 5: identity, source handle, label
const EDGE_5: (&str, Option<&str>, Option<&str>) = ("inbox-documentation", None, None);

pub fn build(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>) -> anyhow::Result<CompiledGraph> {
    build_scope(services, checkpointer, "")
}

pub(super) fn build_scope(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>, scope: &str) -> anyhow::Result<CompiledGraph> {
    let node_0 = START;
    let config_0 = json!({"exports": {"branches": {"documentation": "documentation"}, "contract": {"branches": {"documentation": {"contract": {"input": {"kind": "text"}, "output": {"kind": "text"}}, "invocations": ["node"]}}, "entries": {"main": {"input": {"kind": "text"}, "output": {"kind": "text"}}}}, "entries": {"main": {"inputField": "input", "node": "start", "outputField": "response"}}, "interactive": true}});
    let _ = &config_0;
    let node_1 = NODE_1.0;
    let config_1 = json!({"branch": "documentation", "fanIn": "any", "field": "workingSystemResult", "inputField": "input"});
    let _ = &config_1;
    let node_2 = NODE_2.0;
    let config_2 = json!({"attachments": {"instructions": {"items": [{"id": "restitution", "source": {"kind": "text", "text": "Réponds à l’utilisateur en t’appuyant sur le résultat documentaire fourni par le flow Working System. Préserve ses sources et ses incertitudes."}}]}}, "contextBindings": {"files": {"field": "workingSystemResult", "kind": "state"}, "history": {"historyField": "messages", "inputField": "input", "kind": "conversation"}, "input": {"field": "input", "kind": "state"}, "instructions": {"kind": "attachments", "slot": "instructions"}, "skills": {"kind": "attachments", "slot": "skills"}}, "contextStrategy": "conversation-default", "modelNode": "model"});
    let _ = &config_2;
    let node_3 = NODE_3.0;
    let config_3 = json!({"contextNode": "context", "field": "output", "historyField": "messages", "inputField": "input", "modelBinding": "runtime"});
    let _ = &config_3;
    let node_4 = NODE_4.0;
    let config_4 = json!({"text": "{{output}}"});
    let _ = &config_4;
    let node_5 = NODE_5.0;
    let config_5 = json!({"field": "input", "historyField": "messages", "prompt": "Sur quoi continuer ?", "responseType": "text"});
    let _ = &config_5;
    let channels = json!([{"default": "", "name": "workingSystemResult", "reducer": "overwrite"}]);
    let settings = json!({"idleTimeoutMs": null, "maxConcurrency": 1, "recursionLimit": 10000, "retry": null, "strictChannels": false, "timeoutMs": null});
    let mut runtime_channels: Vec<Value> = serde_json::from_value(channels)?;
    runtime_channels.extend(serde_json::from_value::<Vec<Value>>(json!([{"name": "answer:inbox", "reducer": "overwrite"}, {"name": "__zedflow:answerConsumed:inbox", "reducer": "overwrite"}, {"name": "__zedflow:input:input", "reducer": "overwrite"}, {"name": "__zedflow:model-input:model", "reducer": "overwrite"}, {"name": "__zedflow:prepared-consumed:model", "reducer": "overwrite"}, {"name": "__zedflow:prepared:context", "reducer": "overwrite"}]))?);
    let mut graph = StateGraph::new(operations::state_schema(&Value::Array(runtime_channels))?);
    let path_1 = format!("{scope}{}", node_1);
    let mut runtime_config_1 = if config_1.is_object() { config_1.clone() } else { json!({}) };
    runtime_config_1["nodeId"] = json!(node_1);
    runtime_config_1["__zedflowVersion"] = json!(3);
    graph = graph.add_node_fn(node_1, {
        let services = services.clone();
        move |ctx| {
            let config = runtime_config_1.clone();
            let services = services.clone();
            let path = path_1.clone();
            async move { operations::execute_with_services("route", &config, ctx, &path, services).await }
        }
    });
    let path_2 = format!("{scope}{}", node_2);
    let mut runtime_config_2 = if config_2.is_object() { config_2.clone() } else { json!({}) };
    runtime_config_2["nodeId"] = json!(node_2);
    runtime_config_2["__zedflowVersion"] = json!(3);
    let mut peer_config_2 = config_3.clone();
    peer_config_2["nodeId"] = json!(node_3);
    peer_config_2["__zedflowVersion"] = json!(3);
    graph = graph.add_node(models::context_node_with_services(node_2, &runtime_config_2, &peer_config_2, &path_2, services.clone())?);
    let path_3 = format!("{scope}{}", node_3);
    let mut runtime_config_3 = if config_3.is_object() { config_3.clone() } else { json!({}) };
    runtime_config_3["nodeId"] = json!(node_3);
    runtime_config_3["__zedflowVersion"] = json!(3);
    let mut peer_config_3 = config_2.clone();
    peer_config_3["nodeId"] = json!(node_2);
    peer_config_3["__zedflowVersion"] = json!(3);
    graph = graph.add_node(models::inference_node_with_services(node_3, &runtime_config_3, &peer_config_3, &path_3, services.clone())?);
    let path_4 = format!("{scope}{}", node_4);
    let mut runtime_config_4 = if config_4.is_object() { config_4.clone() } else { json!({}) };
    runtime_config_4["nodeId"] = json!(node_4);
    runtime_config_4["__zedflowVersion"] = json!(3);
    graph = graph.add_node_fn(node_4, {
        let services = services.clone();
        move |ctx| {
            let config = runtime_config_4.clone();
            let services = services.clone();
            let path = path_4.clone();
            async move { operations::execute_with_services("output", &config, ctx, &path, services).await }
        }
    });
    let path_5 = format!("{scope}{}", node_5);
    let mut runtime_config_5 = if config_5.is_object() { config_5.clone() } else { json!({}) };
    runtime_config_5["nodeId"] = json!(node_5);
    runtime_config_5["__zedflowVersion"] = json!(3);
    graph = graph.add_node_fn(node_5, {
        let services = services.clone();
        move |ctx| {
            let config = runtime_config_5.clone();
            let services = services.clone();
            let path = path_5.clone();
            async move { operations::execute_with_services("inbox", &config, ctx, &path, services).await }
        }
    });
    let edge_0 = (node_0, node_1);
    let edge_1 = (node_1, node_2);
    let edge_2 = (node_2, node_3);
    let edge_3 = (node_3, node_4);
    let edge_4 = (node_4, node_5);
    let edge_5 = (node_5, node_1);
    graph = graph.add_edge(edge_0.0, edge_0.1);
    graph = graph.add_edge(edge_1.0, edge_1.1);
    graph = graph.add_edge(edge_2.0, edge_2.1);
    graph = graph.add_edge(edge_3.0, edge_3.1);
    graph = graph.add_edge(edge_4.0, edge_4.1);
    graph.edges.push(Edge::Conditional { source: edge_5.0.to_owned(), router: Arc::new(|_| "next".into()), targets: HashMap::from([("next".into(), EdgeTarget::from(edge_5.1))]) });
    Ok(operations::configure(graph.compile()?, &settings, &[]).with_checkpointer_arc(checkpointer))
}
