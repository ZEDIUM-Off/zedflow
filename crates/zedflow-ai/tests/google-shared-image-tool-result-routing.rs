use serde_json::json;
use zedflow_ai::api::google_shared::{
    AssistantContent, Context, Message, Model, ModelInput, StopReason, UserContent,
    UserContentPart, convert_messages,
};

fn model(id: &str) -> Model {
    Model {
        id: id.into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        input: vec![ModelInput::Text, ModelInput::Image],
    }
}

fn context(model: &Model) -> Context {
    let calls = ["call_a", "call_img", "call_b"]
        .into_iter()
        .map(|id| AssistantContent::ToolCall {
            id: id.into(),
            name: "read".into(),
            arguments: Some(json!({"path": id})),
            thought_signature: None,
        })
        .collect();
    Context {
        messages: vec![
            Message::User {
                content: UserContent::Text("read the files".into()),
            },
            Message::Assistant {
                content: calls,
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                stop_reason: StopReason::ToolUse,
            },
            Message::ToolResult {
                tool_call_id: "call_a".into(),
                tool_name: "read".into(),
                content: vec![UserContentPart::Text {
                    text: "alpha".into(),
                }],
                is_error: false,
            },
            Message::ToolResult {
                tool_call_id: "call_img".into(),
                tool_name: "read".into(),
                content: vec![UserContentPart::Image {
                    data: "abc".into(),
                    mime_type: "image/png".into(),
                }],
                is_error: false,
            },
            Message::ToolResult {
                tool_call_id: "call_b".into(),
                tool_name: "read".into(),
                content: vec![UserContentPart::Text {
                    text: "beta".into(),
                }],
                is_error: false,
            },
        ],
    }
}

#[test]
fn gemini_2_uses_a_synthetic_image_turn() {
    let model = model("gemini-2.5-flash");
    let contents = convert_messages(&model, &context(&model));
    assert_eq!(contents.len(), 5);
    assert!(
        contents[2]
            .parts
            .iter()
            .all(|part| part.function_response.is_some())
    );
    assert_eq!(
        contents[3].parts[0].text.as_deref(),
        Some("Tool result image:")
    );
    assert!(contents[3].parts[1].inline_data.is_some());
    assert!(contents[4].parts[0].function_response.is_some());
}

#[test]
fn gemini_3_nests_images_in_function_responses() {
    let model = model("gemini-3-pro-preview");
    let contents = convert_messages(&model, &context(&model));
    assert_eq!(contents.len(), 3);
    let response = contents[2].parts[1]
        .function_response
        .as_ref()
        .expect("image response");
    assert!(
        response
            .parts
            .as_ref()
            .is_some_and(|parts| parts.len() == 1 && parts[0].inline_data.is_some())
    );
}
