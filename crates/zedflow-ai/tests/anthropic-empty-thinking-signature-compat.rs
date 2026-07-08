use serde_json::{Value, json};
use zedflow_ai::types::{
    AnthropicMessagesCompat, AssistantContentBlock, AssistantMessage, AssistantMessageRole,
    Context, Message, Model, ModelCompat, ModelCost, ModelInput, StopReason, ThinkingContent,
    ThinkingContentType, Usage, UsageCost, UserMessage, UserMessageContent, UserMessageRole,
};

const BLOCKER: &str = "PORT PLACEHOLDER: anthropic-messages request-payload construction and compat::stream_simple on_payload capture are not ported yet; keep ignored until the real payload path exists.";

fn make_model(allow_empty_signature: Option<bool>) -> Model {
    Model {
        id: "mimo-v2.5-pro".to_owned(),
        name: "MiMo-V2.5-Pro".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "xiaomi-token-plan-ams".to_owned(),
        base_url: "http://127.0.0.1:9/anthropic".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 1024,
        headers: None,
        compat: allow_empty_signature.map(|allow_empty_signature| {
            ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
                allow_empty_signature: Some(allow_empty_signature),
                ..AnthropicMessagesCompat::default()
            })
        }),
    }
}

fn usage() -> Usage {
    Usage {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write: 0,
        cache_write_1h: None,
        reasoning: None,
        total_tokens: 0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}

fn make_context(thinking_signature: &str) -> Context {
    let assistant = AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Thinking(ThinkingContent {
            content_type: ThinkingContentType::Thinking,
            thinking: "internal reasoning".to_owned(),
            thinking_signature: Some(thinking_signature.to_owned()),
            redacted: None,
        })],
        api: "anthropic-messages".to_owned(),
        provider: "xiaomi-token-plan-ams".to_owned(),
        model: "mimo-v2.5-pro".to_owned(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: usage(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0,
    };

    Context {
        system_prompt: None,
        messages: vec![
            Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Text("first".to_owned()),
                timestamp: 0,
            }),
            Message::Assistant(assistant),
            Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Text("second".to_owned()),
                timestamp: 0,
            }),
        ],
        tools: None,
    }
}

fn capture_payload(_model: Model, _context: Context) -> Value {
    panic!("{BLOCKER}");
}

fn assistant_content(payload: &Value) -> Option<Value> {
    payload
        .get("messages")?
        .as_array()?
        .iter()
        .find(|message| message.get("role") == Some(&json!("assistant")))?
        .get("content")
        .cloned()
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic payload construction/on_payload capture is not ported"]
fn converts_empty_signature_thinking_to_text_by_default() {
    let payload = capture_payload(make_model(None), make_context(""));
    let assistant = assistant_content(&payload);

    assert_eq!(
        assistant,
        Some(json!([{ "type": "text", "text": "internal reasoning" }]))
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic payload construction/on_payload capture is not ported"]
fn preserves_empty_signature_thinking_when_allow_empty_signature_is_enabled() {
    let payload = capture_payload(make_model(Some(true)), make_context(" "));
    let assistant = assistant_content(&payload);

    assert_eq!(
        assistant,
        Some(json!([{ "type": "thinking", "thinking": "internal reasoning", "signature": "" }]))
    );
}
