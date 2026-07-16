use futures::executor::block_on;
use zedflow_ai::env_api_keys::get_env_api_key;
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AnthropicMessagesCompat, AssistantContentBlock, CacheRetention, Context, Message, Model,
    ModelCompat, ModelCost, ModelInput, SimpleStreamOptions, StopReason, StreamOptions,
    ThinkingLevel, UserMessage, UserMessageContent, UserMessageRole,
};

fn model() -> Model {
    Model {
        id: "claude-opus-4-8".to_owned(),
        name: "Claude Opus 4.8".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url: "https://api.anthropic.com".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000,
        max_tokens: 32_000,
        headers: None,
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            force_adaptive_thinking: Some(true),
            ..AnthropicMessagesCompat::default()
        })),
    }
}

fn context() -> Context {
    Context {
        system_prompt: Some("You are a precise assistant. Follow the user's instructions exactly.".to_owned()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Compute 48291 * 7317 and 90844 - 17729, add the results, and determine whether the sum is divisible by 11. Reply with exactly this format and nothing else: sum=<sum>; divisibleBy11=<yes|no>".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

#[test]
#[ignore = "live capability: requires ANTHROPIC_API_KEY and network"]
fn streams_claude_opus_4_8_with_reasoning_enabled() {
    let Some(api_key) = get_env_api_key("anthropic", None) else {
        return;
    };
    let model = model();
    let context = context();
    let options = SimpleStreamOptions {
        stream: StreamOptions {
            api_key: Some(api_key),
            cache_retention: Some(CacheRetention::None),
            max_tokens: Some(1024),
            ..StreamOptions::default()
        },
        reasoning: Some(ThinkingLevel::High),
        thinking_budgets: None,
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let response = block_on(
        provider
            .stream_simple(&model, &context, Some(&options))
            .result(),
    );
    assert_eq!(
        response.stop_reason,
        StopReason::Stop,
        "{:?}",
        response.error_message
    );
    let thinking = response
        .content
        .iter()
        .find_map(|block| match block {
            AssistantContentBlock::Thinking(thinking) => Some(thinking),
            _ => None,
        })
        .expect("thinking block");
    assert!(
        !thinking
            .thinking_signature
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    );
    let text = response
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text.trim(), "sum=353418362; divisibleBy11=yes");
}
