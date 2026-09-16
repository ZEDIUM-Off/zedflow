// @zedflow format v3.
#![allow(dead_code, unused_imports)]
use adk_graph::prelude::*;
use adk_graph::edge::{Edge, EdgeTarget};
use std::{collections::HashMap, sync::Arc};
use zf_runtime::{models, operations, runtime, subgraphs};

const ZEDFLOW_FORMAT: u32 = 3;
const FLOW: (&str, &str, i64) = ("{{id}}", "{{id}}", 0);
const NODE_0: (&str, &str, &str, f64, f64) = ("start", "flow", "Début", 0.0, 0.0);
const NODE_1: (&str, &str, &str, f64, f64) = ("echo", "flow", "Restituer", 240.0, 0.0);
const NODE_2: (&str, &str, &str, f64, f64) = ("end", "flow", "Fin", 480.0, 0.0);
const EDGE_0: (&str, Option<&str>, Option<&str>) = ("start-echo", None, None);
const EDGE_1: (&str, Option<&str>, Option<&str>) = ("echo-end", None, None);

pub fn build(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>) -> anyhow::Result<CompiledGraph> {
    build_scope(services, checkpointer, "")
}

pub(super) fn build_scope(services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>, scope: &str) -> anyhow::Result<CompiledGraph> {
    let node_0 = START;
    let config_0 = json!({"exports":{"interactive":false,"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}}}});
    let _ = &config_0;
    let node_1 = NODE_1.0;
    let config_1 = json!({"text":"{{input}}"});
    let _ = &config_1;
    let node_2 = END;
    let config_2 = json!({});
    let _ = &config_2;
    let channels = json!([]);
    let settings = json!({"recursionLimit":100,"maxConcurrency":null,"strictChannels":false,"timeoutMs":null,"idleTimeoutMs":null,"retry":null});
    let mut runtime_channels: Vec<Value> = serde_json::from_value(channels)?;
    runtime_channels.extend(serde_json::from_value::<Vec<Value>>(json!([]))?);
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
            async move { operations::execute_with_services("output", &config, ctx, &path, services).await }
        }
    });
    let edge_0 = (node_0, node_1);
    let edge_1 = (node_1, node_2);
    graph = graph.add_edge(edge_0.0, edge_0.1);
    graph = graph.add_edge(edge_1.0, edge_1.1);
    Ok(operations::configure(graph.compile()?, &settings, &[]).with_checkpointer_arc(checkpointer))
}
