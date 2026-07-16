use std::collections::HashSet;

use serde_json::json;
use zedflow_ai::api::openai_responses_shared::{
    AssistantContent, AssistantMessage, Context, Message, Model, StopReason, ToolCall,
    ToolResultMessage, Usage, UserContent, UserMessage, convert_responses_messages,
};
use zedflow_ai::utils::hash::short_hash;

#[test]
fn hashes_foreign_copilot_item_id_into_bounded_codex_shape() {
    let raw = "call_4Vnz|I9b95oN1wD/cHXKTw3PpRkL6KkCtzTJhUxMouMWYwHeTo2j3htzfSk7YPx2vifiIM4g3A8XX";
    let model = Model {
        id: "gpt-5.5".into(),
        api: "openai-responses".into(),
        provider: "openai-codex".into(),
        reasoning: true,
        input: vec!["text".into()],
        cost: Default::default(),
        compat: None,
    };
    let assistant = AssistantMessage {
        content: vec![AssistantContent::ToolCall(ToolCall {
            id: raw.into(),
            name: "edit".into(),
            arguments: json!({"path":"a"}),
            thought_signature: None,
        })],
        api: "openai-responses".into(),
        provider: "github-copilot".into(),
        model: "gpt-5.5".into(),
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::ToolUse,
    };
    let context = Context {
        system_prompt: None,
        messages: vec![
            Message::User(UserMessage {
                content: UserContent::Text("use tool".into()),
            }),
            Message::Assistant(assistant),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: raw.into(),
                tool_name: "edit".into(),
                content: vec![],
                is_error: false,
            }),
        ],
    };
    let input = convert_responses_messages(
        &model,
        &context,
        &HashSet::from(["openai-codex".into()]),
        None,
    );
    let id = input
        .iter()
        .find(|item| item["type"] == "function_call")
        .and_then(|item| item["id"].as_str())
        .expect("function item id");
    let item_id = raw.split_once('|').expect("combined tool id").1;
    assert_eq!(id, format!("fc_{}", short_hash(item_id)));
    assert!(id.len() <= 64);
}
