// @zedflow format v3. See docs/flow-format.md.
#![allow(dead_code, unused_imports)]
use adk_graph::prelude::*;
use adk_graph::edge::{Edge, EdgeTarget};
use std::{collections::HashMap, sync::Arc};
use zf_runtime::{models, operations, runtime, subgraphs};

const ZEDFLOW_FORMAT: u32 = 3;
// @zedflow flow-view: identity, name, revision
const FLOW: (&str, &str, i64) = ("working-system", "Working System", 1);
// @zedflow node-view 0: identity, renderer, label, x, y
const NODE_0: (&str, &str, &str, f64, f64) = ("start", "flow", "Début", 0.0, 140.0);
// @zedflow node-view 1: identity, renderer, label, x, y
const NODE_1: (&str, &str, &str, f64, f64) = ("context", "flow", "Contexte Working System", 220.0, 120.0);
// @zedflow node-view 2: identity, renderer, label, x, y
const NODE_2: (&str, &str, &str, f64, f64) = ("model", "flow", "Modèle documentaire", 510.0, 120.0);
// @zedflow node-view 3: identity, renderer, label, x, y
const NODE_3: (&str, &str, &str, f64, f64) = ("calls", "flow", "Outil demandé ?", 800.0, 100.0);
// @zedflow node-view 4: identity, renderer, label, x, y
const NODE_4: (&str, &str, &str, f64, f64) = ("tools", "flow", "Consulter les sources", 780.0, 360.0);
// @zedflow node-view 5: identity, renderer, label, x, y
const NODE_5: (&str, &str, &str, f64, f64) = ("response", "flow", "Résultat documentaire", 1080.0, 130.0);
// @zedflow node-view 6: identity, renderer, label, x, y
const NODE_6: (&str, &str, &str, f64, f64) = ("end", "flow", "Fin", 1340.0, 140.0);
// @zedflow edge-view 0: identity, source handle, label
const EDGE_0: (&str, Option<&str>, Option<&str>) = ("start-context", None, None);
// @zedflow edge-view 1: identity, source handle, label
const EDGE_1: (&str, Option<&str>, Option<&str>) = ("context-model", None, None);
// @zedflow edge-view 2: identity, source handle, label
const EDGE_2: (&str, Option<&str>, Option<&str>) = ("model-calls", None, None);
// @zedflow edge-view 3: identity, source handle, label
const EDGE_3: (&str, Option<&str>, Option<&str>) = ("calls-tools", Some("true"), None);
// @zedflow edge-view 4: identity, source handle, label
const EDGE_4: (&str, Option<&str>, Option<&str>) = ("calls-response", Some("false"), None);
// @zedflow edge-view 5: identity, source handle, label
const EDGE_5: (&str, Option<&str>, Option<&str>) = ("tools-context", None, None);
// @zedflow edge-view 6: identity, source handle, label
const EDGE_6: (&str, Option<&str>, Option<&str>) = ("response-end", None, None);

pub fn build(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>) -> anyhow::Result<CompiledGraph> {
    build_scope(services, checkpointer, "")
}

pub(super) fn build_scope(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>, scope: &str) -> anyhow::Result<CompiledGraph> {
    let services = services.for_working_directory(Some("../docs"))?;
    let node_0 = START;
    let config_0 = json!({"exports": {"contract": {"entries": {"main": {"input": {"kind": "text"}, "output": {"kind": "text"}}}}, "entries": {"main": {"inputField": "input", "node": "start", "outputField": "response"}}}});
    let _ = &config_0;
    let node_1 = NODE_1.0;
    let config_1 = json!({"attachments": {"instructions": {"items": [{"id": "workspace", "source": {"kind": "workspace"}}, {"id": "mission", "source": {"kind": "text", "text": "Tu interviens dans la documentation du Working System. Réponds à la demande explicite en t’appuyant sur les sources présentes. Le périmètre métier de ce flow reste à préciser ; ne le complète pas par supposition. Cite les fichiers consultés."}}]}, "skills": {"items": [{"activation": "explicit", "id": "skills", "source": {"kind": "workspace"}}]}, "tools": {"items": [{"id": "read", "name": "read"}, {"id": "exec", "name": "exec"}]}}, "contextBindings": {"files": {"kind": "attachments", "slot": "files"}, "history": {"historyField": "messages", "inputField": "input", "kind": "conversation"}, "input": {"field": "input", "kind": "state"}, "instructions": {"kind": "attachments", "slot": "instructions"}, "skills": {"kind": "attachments", "slot": "skills"}}, "contextStrategy": "working-system-context", "fanIn": "any", "modelNode": "model"});
    let _ = &config_1;
    let node_2 = NODE_2.0;
    let config_2 = json!({"contextNode": "context", "field": "output", "historyField": "messages", "inputField": "input", "modelBinding": "runtime", "toolCallsField": "toolCalls"});
    let _ = &config_2;
    let node_3 = NODE_3.0;
    let config_3 = json!({"predicate": {"field": "hasToolCalls", "kind": "compare", "operator": "eq", "value": true}});
    let _ = &config_3;
    let node_4 = NODE_4.0;
    let config_4 = json!({"field": "output", "historyField": "messages", "tool": "execute_calls", "toolCallsField": "toolCalls"});
    let _ = &config_4;
    let node_5 = NODE_5.0;
    let config_5 = json!({"text": "{{output}}"});
    let _ = &config_5;
    let node_6 = END;
    let config_6 = json!({});
    let _ = &config_6;
    let channels = json!([]);
    let settings = json!({"idleTimeoutMs": null, "maxConcurrency": 1, "recursionLimit": 1000, "retry": null, "strictChannels": false, "timeoutMs": null, "workingDirectory": "../docs"});
    let mut runtime_channels: Vec<Value> = serde_json::from_value(channels)?;
    runtime_channels.extend(serde_json::from_value::<Vec<Value>>(json!([{"name": "__zedflow:model-input:model", "reducer": "overwrite"}, {"name": "__zedflow:prepared-consumed:model", "reducer": "overwrite"}, {"name": "__zedflow:prepared:context", "reducer": "overwrite"}, {"name": "__zedflow:condition:calls", "reducer": "overwrite"}]))?);
    let mut graph = StateGraph::new(operations::state_schema(&Value::Array(runtime_channels))?);
    let path_1 = format!("{scope}{}", node_1);
    let mut runtime_config_1 = if config_1.is_object() { config_1.clone() } else { json!({}) };
    runtime_config_1["nodeId"] = json!(node_1);
    runtime_config_1["__zedflowVersion"] = json!(3);
    let mut peer_config_1 = config_2.clone();
    peer_config_1["nodeId"] = json!(node_2);
    peer_config_1["__zedflowVersion"] = json!(3);
    graph = graph.add_node(models::context_node_with_services(node_1, &runtime_config_1, &peer_config_1, &path_1, services.clone())?);
    let path_2 = format!("{scope}{}", node_2);
    let mut runtime_config_2 = if config_2.is_object() { config_2.clone() } else { json!({}) };
    runtime_config_2["nodeId"] = json!(node_2);
    runtime_config_2["__zedflowVersion"] = json!(3);
    let mut peer_config_2 = config_1.clone();
    peer_config_2["nodeId"] = json!(node_1);
    peer_config_2["__zedflowVersion"] = json!(3);
    graph = graph.add_node(models::inference_node_with_services(node_2, &runtime_config_2, &peer_config_2, &path_2, services.clone())?);
    let path_3 = format!("{scope}{}", node_3);
    let mut runtime_config_3 = if config_3.is_object() { config_3.clone() } else { json!({}) };
    runtime_config_3["nodeId"] = json!(node_3);
    runtime_config_3["__zedflowVersion"] = json!(3);
    graph = graph.add_node_fn(node_3, {
        let services = services.clone();
        move |ctx| {
            let config = runtime_config_3.clone();
            let services = services.clone();
            let path = path_3.clone();
            async move { operations::execute_with_services("condition", &config, ctx, &path, services).await }
        }
    });
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
            async move { operations::execute_with_services("tool", &config, ctx, &path, services).await }
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
            async move { operations::execute_with_services("output", &config, ctx, &path, services).await }
        }
    });
    let edge_0 = (node_0, node_1);
    let edge_1 = (node_1, node_2);
    let edge_2 = (node_2, node_3);
    let edge_3 = (node_3, node_4);
    let edge_4 = (node_3, node_5);
    let edge_5 = (node_4, node_1);
    let edge_6 = (node_5, node_6);
    graph = graph.add_edge(edge_0.0, edge_0.1);
    graph = graph.add_edge(edge_1.0, edge_1.1);
    graph = graph.add_edge(edge_2.0, edge_2.1);
    let field_3 = format!("__zedflow:condition:{}", node_3);
    graph.edges.push(Edge::Conditional {
        source: edge_3.0.to_owned(),
        router: Arc::new(move |state| if state.get(&field_3) == Some(&json!(true)) { "true".into() } else { "false".into() }),
        targets: HashMap::from([(EDGE_3.1.unwrap_or("").to_owned(), EdgeTarget::from(edge_3.1)), (EDGE_4.1.unwrap_or("").to_owned(), EdgeTarget::from(edge_4.1))]),
    });
    graph.edges.push(Edge::Conditional { source: edge_5.0.to_owned(), router: Arc::new(|_| "next".into()), targets: HashMap::from([("next".into(), EdgeTarget::from(edge_5.1))]) });
    graph = graph.add_edge(edge_6.0, edge_6.1);
    Ok(operations::configure(graph.compile()?, &settings, &[]).with_checkpointer_arc(checkpointer))
}
