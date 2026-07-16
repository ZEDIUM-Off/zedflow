use serde_json::json;
use zedflow_ai::api::transform_messages::{Message, Model, UserContent, transform_messages};

#[test]
fn normalizes_null_and_missing_content_to_empty_arrays() {
    let messages: Vec<Message> = serde_json::from_value(json!([
        { "role": "user", "content": null, "timestamp": 0 },
        {
            "role": "assistant",
            "content": null,
            "api": "openai-completions",
            "provider": "openai",
            "model": "test-model",
            "responseModel": null,
            "responseId": null,
            "diagnostics": null,
            "usage": {
                "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                "cacheWrite1h": null, "reasoning": null, "totalTokens": 0,
                "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 }
            },
            "stopReason": "stop",
            "errorMessage": null,
            "timestamp": 0
        },
        {
            "role": "toolResult",
            "toolCallId": "call_1",
            "toolName": "web_search",
            "details": null,
            "isError": false,
            "timestamp": 0
        }
    ]))
    .expect("lax messages should deserialize");

    let result = transform_messages(
        &messages,
        &Model {
            id: "test-model".into(),
            api: "openai-completions".into(),
            provider: "openai".into(),
            input: vec!["text".into()],
        },
        None,
    );

    assert_eq!(result.len(), 3);
    match &result[0] {
        Message::User(message) => assert_eq!(message.content, UserContent::Parts(vec![])),
        _ => panic!("expected user message"),
    }
    match &result[1] {
        Message::Assistant(message) => assert!(message.content.is_empty()),
        _ => panic!("expected assistant message"),
    }
    match &result[2] {
        Message::ToolResult(message) => assert!(message.content.is_empty()),
        _ => panic!("expected tool result"),
    }
}
