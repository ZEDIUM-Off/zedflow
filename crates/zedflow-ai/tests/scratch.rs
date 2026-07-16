use zedflow_ai::models::create_models;
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AssistantContentBlock, Context, Message, UserMessage, UserMessageContent, UserMessageRole,
};

const BLOCKER: &str = "live Anthropic scratch script skipped; original requires ANTHROPIC_API_KEY and the Rust port still has provider/auth/completeSimple/streamSimple blockers";

fn scratch_context() -> Context {
    Context {
        system_prompt: Some("You are terse.".to_owned()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Say exactly: ok".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

#[test]
#[ignore = "live Anthropic scratch script requires ANTHROPIC_API_KEY; provider/auth/completeSimple/streamSimple remain blocked"]
fn scratch_models_api_anthropic_smoke_is_live_provider_sample() {
    let mut context = scratch_context();
    assert_eq!(context.system_prompt.as_deref(), Some("You are terse."));

    let mut models = create_models();
    models.set_provider(anthropic_provider().expect(BLOCKER));

    let model = models
        .get_model("anthropic", "claude-haiku-4-5")
        .expect("model not found");
    let auth = models
        .get_auth(&model)
        .expect("auth resolution should not fail")
        .expect("auth should be configured");
    assert!(auth.source.is_some(), "auth should report its source");

    let message = models.complete(&model, &context, None);
    let text = message.content.iter().find_map(|block| match block {
        AssistantContentBlock::Text(text) => Some(text.text.as_str()),
        _ => None,
    });
    assert_eq!(text, Some("ok"));

    context.messages.push(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Text("Now count from 1 to 5, one number per line.".to_owned()),
        timestamp: 0,
    }));

    let stream = models.stream(&model, &context, None);
    assert!(
        !stream.is_done(),
        "stream should start before terminal event"
    );
}
