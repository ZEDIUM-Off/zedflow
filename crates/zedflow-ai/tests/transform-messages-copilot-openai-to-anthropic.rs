use serde_json::json;
use zedflow_ai::api::transform_messages::{
    AssistantContent, AssistantMessage, InputContent, Message, Model, StopReason, TextContent,
    ThinkingContent, ToolCall, UserContent, UserMessage, transform_messages,
};

fn normalize(id: &str, _: &Model, _: &AssistantMessage) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

fn model() -> Model {
    Model {
        id: "claude-sonnet-4.6".into(),
        api: "anthropic-messages".into(),
        provider: "github-copilot".into(),
        input: vec!["text".into(), "image".into()],
    }
}

#[test]
fn copilot_openai_history_is_safe_for_anthropic() {
    let messages = vec![
        Message::User(UserMessage {
            content: UserContent::Text("run it".into()),
            timestamp: 0,
        }),
        Message::Assistant(AssistantMessage {
            content: vec![
                AssistantContent::Thinking(ThinkingContent {
                    thinking: "private".into(),
                    thinking_signature: Some("openai-signature".into()),
                    redacted: false,
                }),
                AssistantContent::ToolCall(ToolCall {
                    id: "call_123|fc_123".into(),
                    name: "bash".into(),
                    arguments: json!({"command":"pwd"}),
                    thought_signature: Some("encrypted".into()),
                }),
            ],
            api: "openai-responses".into(),
            provider: "github-copilot".into(),
            model: "gpt-5".into(),
            stop_reason: StopReason::ToolUse,
            ..AssistantMessage::default()
        }),
    ];
    let result = transform_messages(&messages, &model(), Some(&normalize));
    let Message::Assistant(assistant) = &result[1] else {
        panic!("assistant")
    };
    assert!(
        assistant
            .content
            .iter()
            .all(|block| !matches!(block, AssistantContent::Thinking(_)))
    );
    let tool = assistant
        .content
        .iter()
        .find_map(|block| match block {
            AssistantContent::ToolCall(tool) => Some(tool),
            _ => None,
        })
        .expect("tool");
    assert_eq!(tool.id, "call_123_fc_123");
    assert!(tool.thought_signature.is_none());
    let synthetic = result.last().expect("synthetic result");
    let Message::ToolResult(synthetic) = synthetic else {
        panic!("tool result")
    };
    assert!(synthetic.is_error);
    assert_eq!(synthetic.tool_call_id, tool.id);
    assert!(
        matches!(&synthetic.content[0], InputContent::Text(TextContent { text, .. }) if text == "No result provided")
    );
}
