use std::collections::HashSet;

use serde_json::json;
use zedflow_ai::api::openai_responses_shared::{
    AssistantContent, AssistantMessage, Context, InputContent, Message, Model, ModelCost,
    StopReason, TextContent, ToolCall, ToolResultMessage, Usage, UserContent, UserMessage,
    convert_responses_messages,
};

#[test]
fn uses_no_tool_output_placeholder_for_empty_tool_results_without_images() {
    let model = Model {
        id: "gpt-4o-mini".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        reasoning: false,
        input: vec!["text".to_owned(), "image".to_owned()],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        compat: None,
    };
    let context = Context {
        system_prompt: None,
        messages: vec![
            Message::User(UserMessage {
                content: UserContent::Text("Run the command".to_owned()),
            }),
            Message::Assistant(AssistantMessage {
                content: vec![AssistantContent::ToolCall(ToolCall {
                    id: "tool-1".to_owned(),
                    name: "bash".to_owned(),
                    arguments: json!({ "command": "true" }),
                    thought_signature: None,
                })],
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                response_id: None,
                usage: Usage::default(),
                stop_reason: StopReason::ToolUse,
            }),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "tool-1".to_owned(),
                tool_name: "bash".to_owned(),
                content: vec![InputContent::Text(TextContent {
                    text: String::new(),
                    text_signature: None,
                })],
                is_error: false,
            }),
        ],
    };
    let allowed_tool_call_providers = HashSet::from([
        "openai".to_owned(),
        "openai-codex".to_owned(),
        "opencode".to_owned(),
    ]);

    let input = convert_responses_messages(&model, &context, &allowed_tool_call_providers, None);
    let function_call_output = input
        .iter()
        .find(|item| item["type"] == "function_call_output")
        .expect("function_call_output should be present");
    let output = function_call_output["output"]
        .as_str()
        .expect("function_call_output output should be text");

    assert_eq!(output, "(no tool output)");
    assert!(!output.contains("see attached image"));
}
