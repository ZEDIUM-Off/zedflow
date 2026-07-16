mod common;

use std::collections::HashMap;

use common::http_capture::CapturedRequest;
use serde_json::{Value, json};
use zedflow_ai::api::anthropic_messages::{
    AnthropicEffort, AnthropicOptions, build_request_payload,
};
use zedflow_ai::providers::anthropic_models::{ANTHROPIC_MODELS, AnthropicModel};
use zedflow_ai::types::{
    AnthropicMessagesCompat, Context, Message, Model, ModelCompat, ModelCost, ModelInput,
    ModelThinkingLevel, StreamOptions, ThinkingLevelMap, UserMessage, UserMessageContent,
    UserMessageRole,
};

const LIVE_BLOCKER: &str = "live Anthropic API parity test intentionally ignored: requires ANTHROPIC_API_KEY and provider network calls.";

#[derive(Debug, Clone, Copy)]
enum Reasoning {
    High,
    XHigh,
}

#[derive(Debug, Clone, Copy)]
struct SimpleStreamOptions {
    reasoning: Option<Reasoning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunResult {
    thinking_event_count: usize,
    thinking_char_count: usize,
    text: String,
    content_types: Vec<String>,
}

fn make_payload_capture_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Hello".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

fn model_input(input: &[&str]) -> Vec<ModelInput> {
    input
        .iter()
        .map(|input| match *input {
            "image" => ModelInput::Image,
            _ => ModelInput::Text,
        })
        .collect()
}

fn thinking_level(name: &str) -> ModelThinkingLevel {
    match name {
        "off" => ModelThinkingLevel::Off,
        "minimal" => ModelThinkingLevel::Minimal,
        "low" => ModelThinkingLevel::Low,
        "medium" => ModelThinkingLevel::Medium,
        "high" => ModelThinkingLevel::High,
        "xhigh" => ModelThinkingLevel::XHigh,
        other => panic!("unknown Anthropic thinking level fixture: {other}"),
    }
}

fn thinking_level_map(model: &AnthropicModel) -> Option<ThinkingLevelMap> {
    model.thinking_level_map.map(|entries| {
        entries
            .iter()
            .map(|(level, value)| (thinking_level(level), value.map(str::to_owned)))
            .collect::<HashMap<_, _>>()
    })
}

fn from_anthropic_model(model: &AnthropicModel) -> Model {
    Model {
        id: model.id.to_owned(),
        name: model.name.to_owned(),
        api: model.api.to_owned(),
        provider: model.provider.to_owned(),
        base_url: model.base_url.to_owned(),
        reasoning: model.reasoning,
        thinking_level_map: thinking_level_map(model),
        input: model_input(model.input),
        cost: ModelCost {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
        context_window: u64::from(model.context_window),
        max_tokens: u64::from(model.max_tokens),
        headers: None,
        compat: model.compat.map(|compat| {
            ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
                supports_temperature: compat.supports_temperature,
                force_adaptive_thinking: compat.force_adaptive_thinking,
                ..AnthropicMessagesCompat::default()
            })
        }),
    }
}

fn get_model(_provider: &str, id: &str) -> Model {
    ANTHROPIC_MODELS
        .iter()
        .find(|model| model.id == id)
        .map(from_anthropic_model)
        .expect("anthropic model fixture should exist")
}

fn anthropic_options(options: Option<SimpleStreamOptions>) -> AnthropicOptions {
    let reasoning = options.and_then(|options| options.reasoning);
    AnthropicOptions {
        stream: StreamOptions {
            api_key: Some("fake-key".to_owned()),
            ..StreamOptions::default()
        },
        thinking_enabled: Some(reasoning.is_some()),
        effort: reasoning.map(|reasoning| match reasoning {
            Reasoning::High => AnthropicEffort::High,
            Reasoning::XHigh => AnthropicEffort::XHigh,
        }),
        ..AnthropicOptions::default()
    }
}

fn capture_payload(model: Model, options: Option<SimpleStreamOptions>) -> Value {
    let context = make_payload_capture_context();
    let options = anthropic_options(options);
    let payload = build_request_payload(&model, &context, false, Some(&options));
    CapturedRequest::new("POST", "https://api.anthropic.com/v1/messages")
        .json_body(&payload)
        .body_json()
        .expect("captured Anthropic payload should be JSON")
}

fn make_e2e_context() -> Value {
    json!({
        "systemPrompt": "You are a precise assistant. Follow the requested output format exactly.",
        "messages": [{
            "role": "user",
            "content": "Before replying, carefully solve 36863 * 5279 internally. Then reply with the word pong repeated exactly 40 times, separated by single spaces. Do not add any other text.",
            "timestamp": 0
        }]
    })
}

fn count_pongs(text: &str) -> usize {
    text.split_whitespace()
        .filter(|word| word.eq_ignore_ascii_case("pong"))
        .count()
}

fn run_without_reasoning(model: Model) -> RunResult {
    let _ = (model.id, make_e2e_context());
    panic!("{LIVE_BLOCKER}");
}

#[test]
fn sends_thinking_type_disabled_for_budget_based_reasoning_models_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-sonnet-4-5"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
fn sends_thinking_type_disabled_for_adaptive_reasoning_models_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-opus-4-6"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
fn sends_thinking_type_disabled_for_claude_opus_4_8_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-opus-4-8"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
fn omits_thinking_type_disabled_for_claude_fable_5_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-fable-5"), None);

    assert_eq!(payload.get("thinking"), None);
    assert_eq!(payload.get("output_config"), None);
}

#[test]
fn uses_adaptive_thinking_for_claude_opus_4_8_when_reasoning_is_enabled() {
    let payload = capture_payload(
        get_model("anthropic", "claude-opus-4-8"),
        Some(SimpleStreamOptions {
            reasoning: Some(Reasoning::High),
        }),
    );

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        payload.get("output_config"),
        Some(&json!({ "effort": "high" }))
    );
}

#[test]
fn uses_adaptive_thinking_for_claude_sonnet_5_when_reasoning_is_enabled() {
    let payload = capture_payload(
        get_model("anthropic", "claude-sonnet-5"),
        Some(SimpleStreamOptions {
            reasoning: Some(Reasoning::High),
        }),
    );

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        payload.get("output_config"),
        Some(&json!({ "effort": "high" }))
    );
}

#[test]
fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_opus_4_8() {
    let payload = capture_payload(
        get_model("anthropic", "claude-opus-4-8"),
        Some(SimpleStreamOptions {
            reasoning: Some(Reasoning::XHigh),
        }),
    );

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        payload.get("output_config"),
        Some(&json!({ "effort": "xhigh" }))
    );
}

#[test]
#[ignore = "live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls"]
fn disables_thinking_for_claude_reasoning_models() {
    let result = run_without_reasoning(get_model("anthropic", "claude-sonnet-4-5"));

    assert_eq!(result.thinking_event_count, 0);
    assert_eq!(result.thinking_char_count, 0);
    assert!(!result.content_types.iter().any(|kind| kind == "thinking"));
    assert!(count_pongs(&result.text) >= 35);
}
