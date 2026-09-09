//! An ADK LlmAgent decides whether to enter the research graph or answer.
//! The graph owns the research cycle. The agent has no hidden research tool loop.

use std::{collections::HashMap, sync::Arc};

use adk_agent::LlmAgentBuilder;
use adk_core::{Content, Llm, LlmRequest, LlmResponse, LlmResponseStream, Part};
use adk_graph::{prelude::*, subgraph::SubgraphNode};
use async_trait::async_trait;

/// Offline provider fixture; exercises the real ADK agent without API credentials.
pub struct FixtureModel;

#[async_trait]
impl Llm for FixtureModel {
    fn name(&self) -> &str {
        "zedflow-fixture"
    }

    async fn generate_content(
        &self,
        req: LlmRequest,
        _stream: bool,
    ) -> adk_core::Result<LlmResponseStream> {
        let input = req
            .contents
            .iter()
            .rev()
            .flat_map(|c| &c.parts)
            .find_map(|part| {
                if let Part::Text { text } = part {
                    serde_json::from_str::<Value>(text).ok()
                } else {
                    None
                }
            });
        let has_evidence = input.as_ref().is_some_and(|v| !v["evidence"].is_null());
        let text = if has_evidence {
            "Fixture answer: the research subgraph returned evidence about cycles and isolated channel mappings."
        } else {
            "RESEARCH"
        };
        Ok(Box::pin(futures::stream::iter([Ok(LlmResponse {
            content: Some(Content::new("model").with_text(text)),
            turn_complete: true,
            ..Default::default()
        })])))
    }
}

pub fn build(model: Arc<dyn Llm>) -> anyhow::Result<CompiledGraph> {
    let agent = LlmAgentBuilder::new("decide")
        .model(model)
        .instruction("You receive JSON with question and evidence. If evidence is null, respond with exactly RESEARCH. Otherwise answer the question using that evidence, explicitly stating that the sources are local fixtures. Never invent additional sources.")
        .max_iterations(1)
        .max_output_tokens(512)
        .build()?;
    let decide = AgentNode::new(Arc::new(agent))
        .with_input_mapper(|state| {
            Content::new("user").with_text(
                json!({
                    "question": state.get("question"),
                    "evidence": state.get("evidence"),
                })
                .to_string(),
            )
        })
        .with_output_mapper(|events| {
            let response: String = events
                .iter()
                .filter(|event| !event.llm_response.partial)
                .filter_map(|event| event.llm_response.content.as_ref())
                .flat_map(|content| &content.parts)
                .filter_map(|part| match part {
                    Part::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            HashMap::from([("response".into(), json!(response.trim()))])
        });
    let research = SubgraphNode::new("research", Arc::new(super::research::build()?))
        .isolated()
        .with_input("question", "query")
        .with_output("evidence", "evidence");

    Ok(
        StateGraph::with_channels(&["question", "evidence", "response"])
            .add_node(decide)
            .add_node(research)
            .add_node_fn("validate_response", |ctx| async move {
                if ctx
                    .get("response")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    return Err(GraphError::NodeExecutionFailed {
                        node: "validate_response".into(),
                        message: "agent produced no text; inspect provider errors".into(),
                    });
                }
                Ok(NodeOutput::new())
            })
            .add_edge(START, "decide")
            .add_edge("decide", "validate_response")
            .add_conditional_edges(
                "validate_response",
                |state| {
                    if state.get("response").and_then(Value::as_str) == Some("RESEARCH") {
                        "research".into()
                    } else {
                        END.into()
                    }
                },
                [("research", "research"), (END, END)],
            )
            .add_edge("research", "decide")
            .compile()?
            .with_strict_channels()
            .with_recursion_limit(12),
    )
}
