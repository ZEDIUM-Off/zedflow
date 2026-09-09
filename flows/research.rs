//! Fixture research: validate a query, retrieve local evidence, and return a brief.
//! No network access or claim of real web search. Reusable as an isolated subgraph.

use adk_graph::{error::Result, prelude::*};

pub fn build() -> Result<CompiledGraph> {
    StateGraph::with_channels(&["query", "sources", "evidence"])
        .add_node_fn("validate_query", |ctx| async move {
            if ctx.get("query").and_then(Value::as_str).is_none_or(|q| q.trim().is_empty()) {
                return Err(GraphError::NodeExecutionFailed {
                    node: "validate_query".into(),
                    message: "a non-empty query is required".into(),
                });
            }
            Ok(NodeOutput::new())
        })
        .add_node_fn("retrieve_fixtures", |_ctx| async move {
            Ok(NodeOutput::new().with_update("sources", json!([
                {"uri": "fixture://graph", "text": "ADK graphs support cycles and conditional routing."},
                {"uri": "fixture://subgraph", "text": "SubgraphNode can isolate channels and map inputs and outputs."}
            ])))
        })
        .add_node_fn("prepare_evidence", |ctx| async move {
            let sources = ctx.get("sources").cloned().unwrap_or(json!([]));
            Ok(NodeOutput::new().with_update("evidence", json!({
                "kind": "local_fixture",
                "query": ctx.get("query"),
                "sources": sources,
            })))
        })
        .add_edge(START, "validate_query")
        .add_edge("validate_query", "retrieve_fixtures")
        .add_edge("retrieve_fixtures", "prepare_evidence")
        .add_edge("prepare_evidence", END)
        .compile()
        .map(CompiledGraph::with_strict_channels)
}
