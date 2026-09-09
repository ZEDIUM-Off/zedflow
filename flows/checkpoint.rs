//! Pause after preparing a value, then resume from SQLite in another process.
//! No external side effects: checkpoint restoration is not a filesystem rollback.

use adk_graph::{error::Result, prelude::*};

pub fn build(checkpointer: SqliteCheckpointer, pause: bool) -> Result<CompiledGraph> {
    let graph = StateGraph::with_channels(&["prepared", "preparations", "delivered"])
        .add_node_fn("prepare", |ctx| async move {
            let count = ctx.get("preparations").and_then(Value::as_u64).unwrap_or(0);
            Ok(NodeOutput::new()
                .with_update("prepared", json!("fixture artifact"))
                .with_update("preparations", json!(count + 1)))
        })
        .add_node_fn("deliver", |ctx| async move {
            let prepared =
                ctx.get("prepared")
                    .cloned()
                    .ok_or_else(|| GraphError::NodeExecutionFailed {
                        node: "deliver".into(),
                        message: "missing prepared artifact".into(),
                    })?;
            Ok(NodeOutput::new().with_update("delivered", prepared))
        })
        .add_edge(START, "prepare")
        .add_edge("prepare", "deliver")
        .add_edge("deliver", END)
        .compile()?
        .with_strict_channels()
        .with_checkpointer(checkpointer);
    Ok(if pause {
        graph.with_interrupt_before(&["deliver"])
    } else {
        graph
    })
}
