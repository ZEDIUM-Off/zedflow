use zedflow_ai::api::lazy::Model;
use zedflow_ai::compat::get_model;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, Context, Message, StopReason,
    ThinkingContent, ThinkingLevel, UserMessage, UserMessageContent, UserMessageRole,
};

const BLOCKER: &str = "live Anthropic provider call skipped; compat::get_model/get_models, builtin provider dispatch, anthropic streamSimple payload/onPayload capture, and Anthropic SSE streaming are still PORT PLACEHOLDERs";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThinkingPayload {
    r#type: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputConfig {
    effort: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnthropicThinkingPayload {
    thinking: Option<ThinkingPayload>,
    output_config: Option<OutputConfig>,
}

fn make_context() -> Context {
    Context {
        system_prompt: Some(
            "You are a precise assistant. Follow the user's instructions exactly.".to_owned(),
        ),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text(
                "Compute 48291 * 7317 and 90844 - 17729, add the results, and determine whether the sum is divisible by 11. Reply with exactly this format and nothing else: sum=<sum>; divisibleBy11=<yes|no>".to_owned(),
            ),
            timestamp: 0,
        })],
        tools: None,
    }
}

fn get_smoke_model() -> Result<Model, String> {
    get_model("anthropic", "claude-opus-4-8").map_err(|error| format!("{BLOCKER}: {error}"))
}

fn stream_simple_smoke(
    _model: Model,
    _context: Context,
    _reasoning: ThinkingLevel,
    _max_tokens: u32,
    _on_payload: impl FnOnce(AnthropicThinkingPayload) -> AnthropicThinkingPayload,
) -> Result<(Vec<AssistantMessageEvent>, AssistantMessage), String> {
    Err(BLOCKER.to_owned())
}

fn saw_thinking_event(events: &[AssistantMessageEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            AssistantMessageEvent::ThinkingStart { .. }
                | AssistantMessageEvent::ThinkingDelta { .. }
                | AssistantMessageEvent::ThinkingEnd { .. }
        )
    })
}

fn thinking_block(response: &AssistantMessage) -> Option<&ThinkingContent> {
    response.content.iter().find_map(|block| match block {
        AssistantContentBlock::Thinking(thinking) => Some(thinking),
        _ => None,
    })
}

fn text_response(response: &AssistantMessage) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[test]
#[ignore = "live provider call skipped; compat catalog/builtin dispatch plus anthropic streamSimple/onPayload/SSE are PORT PLACEHOLDERs"]
fn streams_claude_opus_4_8_with_reasoning_enabled() {
    let model = get_smoke_model().expect("getModel should return anthropic claude-opus-4-8");
    let mut captured_payload: Option<AnthropicThinkingPayload> = None;
    let (events, response) = stream_simple_smoke(
        model,
        make_context(),
        ThinkingLevel::High,
        1024,
        |payload| {
            captured_payload = Some(payload.clone());
            payload
        },
    )
    .expect("streamSimple should complete without live/provider placeholder blockers");

    assert_eq!(
        response.stop_reason,
        StopReason::Stop,
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert_eq!(
        captured_payload
            .as_ref()
            .and_then(|payload| payload.thinking.as_ref()),
        Some(&ThinkingPayload { r#type: "adaptive" })
    );
    assert_eq!(
        captured_payload
            .as_ref()
            .and_then(|payload| payload.output_config.as_ref()),
        Some(&OutputConfig { effort: "high" })
    );
    assert!(saw_thinking_event(&events));

    let thinking_block =
        thinking_block(&response).expect("Expected thinking block from Claude Opus 4.8");
    let thinking_signature = thinking_block
        .thinking_signature
        .as_deref()
        .expect("Expected thinking signature from Claude Opus 4.8");
    assert!(!thinking_signature.is_empty());

    assert_eq!(text_response(&response), "sum=353418362; divisibleBy11=yes");
}
