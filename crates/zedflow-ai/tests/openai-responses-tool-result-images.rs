use std::collections::HashSet;

use serde_json::json;
use zedflow_ai::api::openai_responses_shared::{
    AssistantContent, AssistantMessage, Context, ImageContent, InputContent, Message, Model,
    ModelCost, StopReason, TextContent, ToolCall, ToolResultMessage, Usage, UserContent,
    UserMessage, convert_responses_messages,
};

fn image_model() -> Model {
    Model {
        id: "gpt-5-mini".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        reasoning: true,
        input: vec!["text".to_owned(), "image".to_owned()],
        cost: ModelCost::default(),
        compat: None,
    }
}

#[test]
fn sends_tool_result_images_in_function_call_output() {
    let model = image_model();
    let tool_text = "A red circle with a diameter of 100 pixels.";
    let context = Context {
        system_prompt: Some(
            "You are a helpful assistant that always uses the provided tool when asked.".to_owned(),
        ),
        messages: vec![
            Message::User(UserMessage {
                content: UserContent::Text(
                    "Call get_circle_with_description, then describe both the tool text and the image. Mention the color and shape."
                        .to_owned(),
                ),
            }),
            Message::Assistant(AssistantMessage {
                content: vec![AssistantContent::ToolCall(ToolCall {
                    id: "call_get_image|fc_get_image".to_owned(),
                    name: "get_circle_with_description".to_owned(),
                    arguments: json!({}),
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
                tool_call_id: "call_get_image|fc_get_image".to_owned(),
                tool_name: "get_circle_with_description".to_owned(),
                content: vec![
                    InputContent::Text(TextContent {
                        text: tool_text.to_owned(),
                        text_signature: None,
                    }),
                    InputContent::Image(ImageContent {
                        data: "iVBORw0KGgo=".to_owned(),
                        mime_type: "image/png".to_owned(),
                    }),
                ],
                is_error: false,
            }),
        ],
    };

    let response_input = convert_responses_messages(&model, &context, &HashSet::new(), None);
    let function_call_output_index = response_input
        .iter()
        .position(|item| item["type"] == "function_call_output")
        .expect("function_call_output item");
    let function_call_output = &response_input[function_call_output_index];

    let output_items = function_call_output["output"]
        .as_array()
        .expect("function_call_output output to be a content array");
    let text_item = output_items
        .iter()
        .find(|item| item["type"] == "input_text")
        .expect("input_text in function_call_output");
    let image_item = output_items
        .iter()
        .find(|item| item["type"] == "input_image")
        .expect("input_image in function_call_output");

    assert!(
        text_item["text"]
            .as_str()
            .is_some_and(|text| text.contains(tool_text))
    );
    assert!(
        image_item["image_url"]
            .as_str()
            .is_some_and(|url| url.starts_with("data:image/png;base64,"))
    );

    let later_user_messages = response_input
        .iter()
        .skip(function_call_output_index + 1)
        .filter(|item| item["role"] == "user")
        .count();
    assert_eq!(later_user_messages, 0);
}
