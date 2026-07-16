mod common;

use common::http_capture::CapturedRequest;
use zedflow_ai::api::anthropic_messages::{AnthropicOptions, build_request_payload};
use zedflow_ai::providers::anthropic_models::{ANTHROPIC_MODELS, AnthropicModel};
use zedflow_ai::types::{
    AnthropicMessagesCompat, Context, Message, Model, ModelCompat, ModelCost, ModelInput,
    StreamOptions, UserMessage, UserMessageContent, UserMessageRole,
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnthropicTemperaturePayload {
    temperature: Option<f64>,
}

fn make_context() -> Context {
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

fn from_anthropic_model(model: &AnthropicModel) -> Model {
    Model {
        id: model.id.to_owned(),
        name: model.name.to_owned(),
        api: model.api.to_owned(),
        provider: model.provider.to_owned(),
        base_url: model.base_url.to_owned(),
        reasoning: model.reasoning,
        thinking_level_map: None,
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

fn get_anthropic_model(id: &str) -> Model {
    ANTHROPIC_MODELS
        .iter()
        .find(|model| model.id == id)
        .map(from_anthropic_model)
        .expect("anthropic model fixture should exist")
}

fn make_custom_model(compat: Option<AnthropicMessagesCompat>) -> Model {
    Model {
        id: "vendor--claude-opus-4-7".to_owned(),
        name: "Vendor Proxy Opus 4.7".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "vendor-proxy".to_owned(),
        base_url: "http://127.0.0.1:9".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 32_000,
        headers: None,
        compat: compat.map(ModelCompat::AnthropicMessages),
    }
}

fn options(temperature: f64) -> AnthropicOptions {
    AnthropicOptions {
        stream: StreamOptions {
            temperature: Some(temperature),
            api_key: Some("fake-key".to_owned()),
            ..StreamOptions::default()
        },
        ..AnthropicOptions::default()
    }
}

fn capture_payload(model: Model, options: Option<AnthropicOptions>) -> AnthropicTemperaturePayload {
    let context = make_context();
    let payload = build_request_payload(&model, &context, false, options.as_ref());
    let payload = CapturedRequest::new("POST", "http://127.0.0.1:9/v1/messages")
        .json_body(&payload)
        .body_json()
        .expect("captured Anthropic payload should be JSON");

    AnthropicTemperaturePayload {
        temperature: payload
            .get("temperature")
            .and_then(serde_json::Value::as_f64),
    }
}

#[test]
fn omits_temperature_for_claude_opus_4_7() {
    let payload = capture_payload(get_anthropic_model("claude-opus-4-7"), Some(options(0.0)));

    assert_eq!(payload.temperature, None);
}

#[test]
fn omits_temperature_for_claude_opus_4_8() {
    let payload = capture_payload(get_anthropic_model("claude-opus-4-8"), Some(options(0.0)));

    assert_eq!(payload.temperature, None);
}

#[test]
fn omits_default_temperature_for_claude_opus_4_7() {
    let payload = capture_payload(get_anthropic_model("claude-opus-4-7"), Some(options(1.0)));

    assert_eq!(payload.temperature, None);
}

#[test]
fn keeps_temperature_for_claude_opus_4_6() {
    let payload = capture_payload(get_anthropic_model("claude-opus-4-6"), Some(options(0.0)));

    assert_eq!(payload.temperature, Some(0.0));
}

#[test]
fn keeps_temperature_for_claude_sonnet_4_6() {
    let payload = capture_payload(get_anthropic_model("claude-sonnet-4-6"), Some(options(0.0)));

    assert_eq!(payload.temperature, Some(0.0));
}

#[test]
fn omits_temperature_for_custom_models_with_supports_temperature_disabled() {
    let payload = capture_payload(
        make_custom_model(Some(AnthropicMessagesCompat {
            supports_temperature: Some(false),
            ..AnthropicMessagesCompat::default()
        })),
        Some(options(0.0)),
    );

    assert_eq!(payload.temperature, None);
}
