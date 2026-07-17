use std::collections::HashMap;

use zedflow_ai::api::transform_messages::transform_messages;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageRole, Message, Model, ModelInput,
    StopReason, TextContentType, ThinkingContent, ThinkingContentType, ToolCall, ToolCallType,
    ToolResultContentBlock, Usage, UserMessage, UserMessageContent, UserMessageRole,
};

fn normalize(id: &str, _: &Model, _: &AssistantMessage) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
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
        input: vec![ModelInput::Text, ModelInput::Image],
        ..Model::default()
    }
}

#[test]
fn copilot_openai_history_is_safe_for_anthropic() {
    let messages = vec![
        Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("run it".into()),
            timestamp: 0,
        }),
        Message::Assistant(AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![
                AssistantContentBlock::Thinking(ThinkingContent {
                    content_type: ThinkingContentType::Thinking,
                    thinking: "private".into(),
                    thinking_signature: Some("openai-signature".into()),
                    redacted: Some(false),
                }),
                AssistantContentBlock::ToolCall(ToolCall {
                    content_type: ToolCallType::ToolCall,
                    id: "call_123|fc_123".into(),
                    name: "bash".into(),
                    arguments: HashMap::from([("command".into(), "pwd".into())]),
                    thought_signature: Some("encrypted".into()),
                }),
            ],
            api: "openai-responses".into(),
            provider: "github-copilot".into(),
            model: "gpt-5".into(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: StopReason::ToolUse,
            error_message: None,
            timestamp: 0,
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
            .all(|block| !matches!(block, AssistantContentBlock::Thinking(_)))
    );
    let tool = assistant
        .content
        .iter()
        .find_map(|block| match block {
            AssistantContentBlock::ToolCall(tool) => Some(tool),
            _ => None,
        })
        .expect("tool");
    assert_eq!(tool.id, "call_123_fc_123");
    assert!(tool.thought_signature.is_none());
    let Message::ToolResult(synthetic) = result.last().expect("synthetic result") else {
        panic!("tool result")
    };
    assert!(synthetic.is_error);
    assert_eq!(synthetic.tool_call_id, tool.id);
    assert!(
        matches!(&synthetic.content[0], ToolResultContentBlock::Text(text)
        if text.content_type == TextContentType::Text && text.text == "No result provided")
    );
}
