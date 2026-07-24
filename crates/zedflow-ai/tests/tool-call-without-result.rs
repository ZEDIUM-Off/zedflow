use std::collections::HashMap;

use zedflow_ai::api::transform_messages::transform_messages;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageRole, Message, Model, ModelInput,
    StopReason, TextContent, TextContentType, ToolCall, ToolCallType, ToolResultContentBlock,
    Usage, UserMessage, UserMessageContent, UserMessageRole,
};

fn destination_model() -> Model {
    Model {
        id: "gpt-5-mini".into(),
        api: "openai-responses".into(),
        provider: "openai".into(),
        input: vec![ModelInput::Text],
        ..Model::default()
    }
}

fn user(content: &str) -> Message {
    Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Text(content.into()),
        timestamp: 0,
    })
}

fn assistant_tool_call() -> Message {
    Message::Assistant(AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::ToolCall(ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "call_calculate".into(),
            name: "calculate".into(),
            arguments: HashMap::from([("expression".into(), "25 * 18".into())]),
            thought_signature: None,
        })],
        api: "openai-responses".into(),
        provider: "openai".into(),
        model: "gpt-5-mini".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::ToolUse,
        error_message: None,
        timestamp: 0,
    })
}

#[test]
fn inserts_missing_tool_result_before_followup_user_message() {
    let result = transform_messages(
        &[
            user("Please calculate 25 * 18 using the calculate tool."),
            assistant_tool_call(),
            user("Never mind, just tell me what is 2+2?"),
        ],
        &destination_model(),
        None,
    );

    assert_eq!(result.len(), 4);
    let Message::ToolResult(synthetic) = &result[2] else {
        panic!("synthetic tool result")
    };
    assert_eq!(synthetic.tool_call_id, "call_calculate");
    assert_eq!(synthetic.tool_name, "calculate");
    assert!(synthetic.is_error);
    assert_eq!(
        synthetic.content,
        vec![ToolResultContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: "No result provided".into(),
            text_signature: None,
        })]
    );
    assert!(matches!(&result[3], Message::User(message)
        if message.content == UserMessageContent::Text("Never mind, just tell me what is 2+2?".into())));
}

#[test]
fn inserts_missing_tool_result_at_end_of_context() {
    let result = transform_messages(&[assistant_tool_call()], &destination_model(), None);
    assert!(matches!(result.last(), Some(Message::ToolResult(value))
        if value.tool_call_id == "call_calculate" && value.is_error));
}
