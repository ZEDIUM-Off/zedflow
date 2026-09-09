//! Share one ADK memory service between graph assemblies, with project isolation.
//! The service outlives individual runs; it remains in memory and dies with the process.

use std::sync::Arc;

use adk_core::{Content, Part};
use adk_graph::prelude::*;
use adk_memory::{InMemoryMemoryService, MemoryEntry, MemoryService, SearchRequest};

pub fn build(service: Arc<InMemoryMemoryService>, project: &str) -> anyhow::Result<CompiledGraph> {
    adk_memory::validate_project_id(project)?;
    let writer = Arc::clone(&service);
    let write_project = project.to_owned();
    let read_project = project.to_owned();
    Ok(StateGraph::with_channels(&["note", "query", "matches"])
        .add_node_fn("remember", move |ctx| {
            let service = Arc::clone(&writer);
            let project = write_project.clone();
            async move {
                if let Some(note) = ctx.get("note").and_then(Value::as_str) {
                    service
                        .add_session_to_project(
                            "zedflow-lab",
                            "lab-user",
                            &ctx.config.thread_id,
                            &project,
                            vec![MemoryEntry {
                                content: Content::new("user").with_text(note),
                                author: "remember".into(),
                                timestamp: std::time::SystemTime::now().into(),
                            }],
                        )
                        .await
                        .map_err(|error| GraphError::NodeExecutionFailed {
                            node: "remember".into(),
                            message: error.to_string(),
                        })?;
                }
                Ok(NodeOutput::new())
            }
        })
        .add_node_fn("recall", move |ctx| {
            let service = Arc::clone(&service);
            let project = read_project.clone();
            async move {
                let query = ctx.get("query").and_then(Value::as_str).unwrap_or("graph");
                let response = service
                    .search(SearchRequest {
                        query: query.into(),
                        user_id: "lab-user".into(),
                        app_name: "zedflow-lab".into(),
                        limit: Some(10),
                        min_score: None,
                        project_id: Some(project),
                    })
                    .await
                    .map_err(|error| GraphError::NodeExecutionFailed {
                        node: "recall".into(),
                        message: error.to_string(),
                    })?;
                let texts: Vec<_> = response
                    .memories
                    .iter()
                    .flat_map(|m| &m.content.parts)
                    .filter_map(|part| match part {
                        Part::Text { text } => Some(text),
                        _ => None,
                    })
                    .collect();
                Ok(NodeOutput::new().with_update("matches", json!(texts)))
            }
        })
        .add_edge(START, "remember")
        .add_edge("remember", "recall")
        .add_edge("recall", END)
        .compile()?
        .with_strict_channels())
}
