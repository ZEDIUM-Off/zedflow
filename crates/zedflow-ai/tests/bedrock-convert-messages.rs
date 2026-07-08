//! Port of Pi `packages/ai/test/bedrock-convert-messages.test.ts`.
//!
//! PORT PLACEHOLDER: Bedrock Converse payload construction and `on_payload` capture are not
//! ported in `zedflow_ai::api::bedrock_converse_stream::stream` yet. Keep these parity tests
//! ignored until the real local conversion path exists; they make no live provider calls.

use serde_json::{Value, json};

const BLOCKER: &str =
    "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported yet";

fn capture_payload(_context: Value) -> Value {
    panic!("{BLOCKER}");
}

fn assistant_message(content: Value) -> Value {
    json!({
        "role": "assistant",
        "content": content,
        "api": "bedrock-converse-stream",
        "provider": "amazon-bedrock",
        "model": "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
        "usage": {
            "input": 0,
            "output": 0,
            "cacheRead": 0,
            "cacheWrite": 0,
            "totalTokens": 0,
            "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 }
        },
        "stopReason": "stop",
        "timestamp": 0
    })
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn skips_unknown_user_content_blocks_instead_of_throwing() {
    let payload = capture_payload(json!({
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "hello" },
                { "type": "unknown", "data": "foo" }
            ],
            "timestamp": 0
        }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        payload["messages"][0]["content"][0],
        json!({ "text": "hello" })
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn skips_unknown_assistant_content_blocks_instead_of_throwing() {
    let payload = capture_payload(json!({
        "messages": [assistant_message(json!([
            { "type": "text", "text": "hello" },
            { "type": "unknown", "data": "foo" }
        ]))]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        payload["messages"][0]["content"][0],
        json!({ "text": "hello" })
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn replaces_user_messages_with_only_unknown_content_blocks_with_a_placeholder() {
    let payload = capture_payload(json!({
        "messages": [{
            "role": "user",
            "content": [{ "type": "unknown", "data": "foo" }],
            "timestamp": 0
        }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"],
        json!([{ "text": "<empty>" }])
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn replaces_blank_user_string_content_with_a_placeholder() {
    let payload = capture_payload(json!({
        "messages": [{ "role": "user", "content": "   ", "timestamp": 0 }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"],
        json!([{ "text": "<empty>" }])
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn filters_blank_user_text_blocks_when_other_content_remains() {
    let payload = capture_payload(json!({
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "" },
                { "type": "text", "text": "hello" }
            ],
            "timestamp": 0
        }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"],
        json!([{ "text": "hello" }])
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn replaces_user_content_emptied_by_surrogate_sanitization_with_a_placeholder() {
    let context: Value =
        serde_json::from_str(r#"{"messages":[{"role":"user","content":"\ud83d","timestamp":0}]}"#)
            .expect("fixture JSON should parse");
    let payload = capture_payload(context);

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"],
        json!([{ "text": "<empty>" }])
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn skips_assistant_text_blocks_emptied_by_surrogate_sanitization() {
    let message = assistant_message(
        serde_json::from_str(r#"[{"type":"text","text":"\ud83d"}]"#)
            .expect("fixture JSON should parse"),
    );
    let payload = capture_payload(json!({ "messages": [message] }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(0));
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn replaces_blank_tool_result_content_with_a_placeholder() {
    let payload = capture_payload(json!({
        "messages": [{
            "role": "toolResult",
            "toolCallId": "tool-1",
            "toolName": "tool",
            "content": [{ "type": "text", "text": "" }],
            "isError": false,
            "timestamp": 0
        }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"][0]["toolResult"]["content"],
        json!([{ "text": "<empty>" }])
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock Converse payload construction/on_payload capture is not ported"]
fn skips_assistant_messages_with_only_unknown_content_blocks() {
    let payload = capture_payload(json!({
        "messages": [assistant_message(json!([{ "type": "unknown", "data": "foo" }]))]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(0));
}
