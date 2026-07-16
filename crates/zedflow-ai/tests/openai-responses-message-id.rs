use std::collections::HashSet;

use zedflow_ai::api::openai_responses_shared::{
    AssistantContent, AssistantMessage, Context, Message, Model, StopReason, TextContent,
    ThinkingContent, Usage, UserContent, UserMessage, convert_responses_messages,
};

#[test]
fn fallback_message_ids_are_unique_per_text_block() {
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
        content: vec![
            AssistantContent::Thinking(ThinkingContent {
                thinking: "private".into(),
                thinking_signature: None,
                redacted: false,
            }),
            AssistantContent::Text(TextContent {
                text: "private".into(),
                text_signature: None,
            }),
            AssistantContent::Text(TextContent {
                text: "visible".into(),
                text_signature: None,
            }),
        ],
        api: "anthropic-messages".into(),
        provider: "anthropic".into(),
        model: "claude".into(),
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
    };
    let input = convert_responses_messages(
        &model,
        &Context {
            system_prompt: None,
            messages: vec![
                Message::User(UserMessage {
                    content: UserContent::Text("hi".into()),
                }),
                Message::Assistant(assistant),
            ],
        },
        &HashSet::from(["openai-codex".into()]),
        None,
    );
    let ids: Vec<_> = input
        .iter()
        .filter(|item| item["type"] == "message")
        .filter_map(|item| item["id"].as_str())
        .collect();
    assert_eq!(ids, ["msg_pi_1", "msg_pi_1_1", "msg_pi_1_2"]);
}
