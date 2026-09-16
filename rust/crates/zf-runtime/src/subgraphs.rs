//! Occurrence-scoped use of ADK's native subgraph node.
//!
//! ADK persists each child's frontier. This adapter distinguishes a new visit in
//! a parent loop from re-entering the same paused visit, and prevents projection
//! of the initial input from overwriting the child's checkpointed input.
use adk_graph::{
    CompiledGraph, Node, NodeContext, NodeOutput, State, StateSchema,
    checkpoint::Checkpointer,
    error::{GraphError, Result},
    subgraph::SubgraphNode,
};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};

/// IDs acknowledged by durable graph state, including children that have not
/// returned to their parent yet. This enriches the application's run snapshot;
/// it must not replace or rewrite ADK checkpoints.
pub async fn collect_consumed_messages(
    checkpointer: &dyn Checkpointer,
    thread_id: &str,
    root_state: &State,
) -> Result<Vec<String>> {
    let mut consumed = BTreeSet::new();
    let collect = |state: &State, ids: &mut BTreeSet<String>| {
        ids.extend(
            state
                .get("__zedflow:consumedMessages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
    };
    collect(root_state, &mut consumed);
    let mut pending = vec![thread_id.to_owned()];
    while let Some(thread) = pending.pop() {
        let Some(checkpoint) = checkpointer.load(&thread).await? else {
            continue;
        };
        collect(&checkpoint.state, &mut consumed);
        // Only the active frontier can hold a child which has not returned its
        // output. Finished children have already projected their consumed IDs.
        pending.extend(
            checkpoint
                .pending_nodes
                .iter()
                .map(|node| format!("{thread}/{node}@{}", checkpoint.step)),
        );
    }
    Ok(consumed.into_iter().collect())
}

pub struct ResumableSubgraph {
    id: String,
    graph: Arc<CompiledGraph>,
    answers: Vec<String>,
}

impl ResumableSubgraph {
    pub fn new(id: &str, graph: Arc<CompiledGraph>, answers: Vec<String>) -> Self {
        Self {
            id: id.into(),
            graph,
            answers,
        }
    }

    fn native(&self, name: String) -> SubgraphNode {
        let mut node = SubgraphNode::new(name, self.graph.clone())
            .isolated()
            .with_input("input", "input")
            .with_output("response", "output")
            .with_input("__zedflow:consumedMessages", "__zedflow:consumedMessages")
            .with_output("__zedflow:consumedMessages", "__zedflow:consumedMessages");
        for answer in &self.answers {
            node = node
                .with_input(
                    format!("answer:{}/{answer}", self.id),
                    format!("answer:{answer}"),
                )
                .with_output(
                    format!("answer:{answer}"),
                    format!("answer:{}/{answer}", self.id),
                );
        }
        node
    }
}

#[async_trait]
impl Node for ResumableSubgraph {
    fn name(&self) -> &str {
        &self.id
    }

    fn validate_against(&self, parent: &StateSchema) -> Result<()> {
        if !self.graph.has_checkpointer() {
            return Err(GraphError::InvalidGraph(format!(
                "Sous-graphe {} : checkpointer requis",
                self.id
            )));
        }
        self.native(self.id.clone()).validate_against(parent)
    }

    async fn execute(&self, ctx: &NodeContext) -> Result<NodeOutput> {
        let name = format!("{}@{}", self.id, ctx.step);
        let thread = format!("{}/{name}", ctx.config.thread_id);
        let mut state = ctx.state.clone();
        if let Some(checkpointer) = self.graph.checkpointer()
            && let Some(saved) = checkpointer.load(&thread).await?
        {
            match saved.state.get("input") {
                Some(value) => {
                    state.insert("input".into(), value.clone());
                }
                None => {
                    state.remove("input");
                }
            }
            let mut consumed = saved
                .state
                .get("__zedflow:consumedMessages")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for id in state
                .get("__zedflow:consumedMessages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if !consumed.contains(id) {
                    consumed.push(id.clone());
                }
            }
            state.insert("__zedflow:consumedMessages".into(), json!(consumed));
        }
        let mut adapted = NodeContext::new(state, ctx.config.clone(), ctx.step);
        let schema = ctx
            .parent_schema()
            .ok_or_else(|| GraphError::InvalidGraph("Schéma parent absent".into()))?;
        adapted.set_parent_schema(schema);
        self.native(name).execute(&adapted).await
    }
}
