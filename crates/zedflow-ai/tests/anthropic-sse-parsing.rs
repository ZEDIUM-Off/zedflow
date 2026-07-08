//! Deterministic raw Anthropic SSE parity fixtures.

use serde_json::{Value, json};
use zedflow_ai::api::anthropic_messages::assistant_message_from_sse;
use zedflow_ai::types::{
    AssistantContentBlock, Context, Message, Model, ModelCost, ModelInput, StopReason, Tool,
    UserMessage, UserMessageContent, UserMessageRole,
};

fn test_model() -> Model {
    Model {
        id: "claude-haiku-4-5".to_owned(),
        name: "Claude Haiku 4.5".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url: "https://api.anthropic.com".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    }
}

fn test_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Say hello.".to_owned()),
            timestamp: 0,
        })],
        tools: Some(vec![Tool {
            name: "edit".to_owned(),
            description: "Edit a file".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["path", "text"]
            }),
        }]),
    }
}

fn sse(event: &str, data: Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

fn minimal_anthropic_events() -> String {
    format!(
        "{}{}{}{}{}{}",
        sse(
            "message_start",
            json!({
                "type": "message_start",
                "message": {
                    "id": "msg_test",
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 0,
                        "cache_read_input_tokens": 0,
                        "cache_creation_input_tokens": 0
                    }
                }
            })
        ),
        sse(
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "text", "text": "" }
            })
        ),
        sse(
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "text_delta", "text": "Hello" }
            })
        ),
        sse(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 })
        ),
        sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn" },
                "usage": { "output_tokens": 5 }
            })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    )
}

#[test]
fn repairs_malformed_sse_json_and_malformed_streamed_tool_json() {
    let malformed_tool_json_delta = "{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"A\\H\\\",\\\"text\\\":\\\"col1\tcol2\\\"}\"}}";
    let raw = format!(
        "{}{}event: content_block_delta\ndata: {malformed_tool_json_delta}\n\n{}{}{}",
        sse(
            "message_start",
            json!({
                "type": "message_start",
                "message": { "id": "msg_test", "usage": { "input_tokens": 12 } }
            })
        ),
        sse(
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "tool_use", "id": "toolu_test", "name": "edit", "input": {} }
            })
        ),
        sse(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 })
        ),
        sse(
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 5 } })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::ToolUse);
    let AssistantContentBlock::ToolCall(tool_call) = &result.content[0] else {
        panic!("expected tool call");
    };
    assert_eq!(tool_call.arguments.get("path"), Some(&json!("A\\H")));
    assert_eq!(tool_call.arguments.get("text"), Some(&json!("col1\tcol2")));
}

#[test]
fn preserves_refusal_stop_details_from_message_delta() {
    let explanation = "This request triggered restrictions.";
    let raw = format!(
        "{}{}{}",
        sse(
            "message_start",
            json!({ "type": "message_start", "message": { "id": "msg_01", "usage": { "input_tokens": 412 } } })
        ),
        sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": "refusal",
                    "stop_details": { "type": "refusal", "category": "cyber", "explanation": explanation }
                },
                "usage": { "output_tokens": 0 }
            })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::Error);
    assert_eq!(result.error_message.as_deref(), Some(explanation));
}

#[test]
fn ignores_unknown_sse_events_after_message_stop() {
    let raw = format!(
        "{}event: done\ndata: [DONE]\n\nevent: proxy.stats\ndata: not json\n\n",
        minimal_anthropic_events()
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::Stop);
    assert_eq!(result.error_message, None);
    let AssistantContentBlock::Text(text) = &result.content[0] else {
        panic!("expected text");
    };
    assert_eq!(text.text, "Hello");
}
