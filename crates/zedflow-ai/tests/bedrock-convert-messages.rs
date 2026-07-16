//! Port of Pi `packages/ai/test/bedrock-convert-messages.test.ts`.
//!
//! Uses the deterministic Bedrock request-plan payload seam; no live AWS calls are made.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::bedrock_converse_stream::{
    BedrockOptions, CacheRetention, Model, build_bedrock_converse_payload,
};

fn model() -> Model {
    Model {
        id: "us.anthropic.claude-sonnet-4-5-20250929-v1:0".to_owned(),
        provider: "amazon-bedrock".to_owned(),
        name: None,
        base_url: None,
        max_tokens: 4096,
        reasoning: true,
        thinking_level_map: HashMap::new(),
    }
}

fn capture_payload(context: Value) -> Value {
    build_bedrock_converse_payload(
        &model(),
        &context,
        &BedrockOptions {
            cache_retention: Some(CacheRetention::None),
            ..Default::default()
        },
    )
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
fn replaces_user_content_emptied_by_surrogate_sanitization_with_a_placeholder() {
    // Rust strings cannot contain Pi's lone UTF-16 surrogate; empty text exercises the same
    // post-sanitization Bedrock placeholder branch.
    let payload = capture_payload(json!({
        "messages": [{ "role": "user", "content": "", "timestamp": 0 }]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        payload["messages"][0]["content"],
        json!([{ "text": "<empty>" }])
    );
}

#[test]
fn skips_assistant_text_blocks_emptied_by_surrogate_sanitization() {
    // Rust strings cannot contain Pi's lone UTF-16 surrogate; empty text exercises the same
    // post-sanitization Bedrock filtering branch.
    let message = assistant_message(json!([{ "type": "text", "text": "" }]));
    let payload = capture_payload(json!({ "messages": [message] }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(0));
}

#[test]
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
fn skips_assistant_messages_with_only_unknown_content_blocks() {
    let payload = capture_payload(json!({
        "messages": [assistant_message(json!([{ "type": "unknown", "data": "foo" }]))]
    }));

    assert_eq!(payload["messages"].as_array().map(Vec::len), Some(0));
}
