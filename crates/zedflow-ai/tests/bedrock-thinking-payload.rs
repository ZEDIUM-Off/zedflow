use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::bedrock_converse_stream::{BedrockOptions, Model, ThinkingLevel};

const PAYLOAD_BLOCKER: &str = "PORT PLACEHOLDER: bedrock-converse-stream request-payload construction/on_payload capture is not ported; keep ignored until the real ConverseStream payload path exists.";
const LIVE_BLOCKER: &str = "live Bedrock Claude parity test intentionally ignored: requires AWS Bedrock credentials and provider network calls.";

fn get_model(provider: &str, id: &str) -> Model {
    Model {
        id: id.to_owned(),
        provider: provider.to_owned(),
        name: Some(
            match id {
                "global.anthropic.claude-fable-5" => "Claude Fable 5",
                "global.anthropic.claude-sonnet-5" => "Claude Sonnet 5",
                "global.anthropic.claude-sonnet-4-6" => "Claude Sonnet 4.6",
                "global.anthropic.claude-opus-4-6-v1" => "Claude Opus 4.6 (Global)",
                "us.anthropic.claude-sonnet-4-5-20250929-v1:0" => "Claude Sonnet 4.5",
                _ => id,
            }
            .to_owned(),
        ),
        base_url: None,
        max_tokens: 4096,
        reasoning: true,
        thinking_level_map: HashMap::new(),
    }
}

fn with_id_and_name(mut model: Model, id: &str, name: &str) -> Model {
    model.id = id.to_owned();
    model.name = Some(name.to_owned());
    model
}

fn make_context() -> Value {
    json!({
        "messages": [{ "role": "user", "content": "Hello", "timestamp": 0 }]
    })
}

fn options(reasoning: Option<ThinkingLevel>, region: Option<&str>) -> BedrockOptions {
    BedrockOptions {
        reasoning,
        region: region.map(str::to_owned),
        ..Default::default()
    }
}

fn capture_payload(model: &Model, options: Option<BedrockOptions>) -> Value {
    let mut options = options.unwrap_or_else(|| BedrockOptions {
        reasoning: Some(ThinkingLevel::High),
        ..Default::default()
    });
    if options.reasoning.is_none() {
        options.reasoning = Some(ThinkingLevel::High);
    }
    let _ = (model, make_context(), options);
    panic!("{PAYLOAD_BLOCKER}");
}

fn additional_field<'a>(payload: &'a Value, name: &str) -> Option<&'a Value> {
    payload.get("additionalModelRequestFields")?.get(name)
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn uses_adaptive_thinking_for_claude_opus_4_8_when_reasoning_is_enabled() {
    let base_model = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1");
    let model = with_id_and_name(
        base_model,
        "global.anthropic.claude-opus-4-8-v1",
        "Claude Opus 4.8 (Global)",
    );

    let payload = capture_payload(&model, None);

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "high" }))
    );
    assert_eq!(additional_field(&payload, "anthropic_beta"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_opus_4_8() {
    let base_model = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1");
    let model = with_id_and_name(
        base_model,
        "global.anthropic.claude-opus-4-8-v1",
        "Claude Opus 4.8 (Global)",
    );

    let payload = capture_payload(&model, Some(options(Some(ThinkingLevel::XHigh), None)));

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "xhigh" }))
    );
    assert_eq!(additional_field(&payload, "anthropic_beta"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn uses_adaptive_thinking_for_claude_fable_5_when_reasoning_is_enabled() {
    let model = get_model("amazon-bedrock", "global.anthropic.claude-fable-5");

    let payload = capture_payload(&model, None);

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "high" }))
    );
    assert_eq!(additional_field(&payload, "anthropic_beta"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn uses_adaptive_thinking_for_claude_sonnet_5_when_reasoning_is_enabled() {
    let model = get_model("amazon-bedrock", "global.anthropic.claude-sonnet-5");

    let payload = capture_payload(&model, None);

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "high" }))
    );
    assert_eq!(additional_field(&payload, "anthropic_beta"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn maps_xhigh_reasoning_to_effort_xhigh_for_claude_fable_5() {
    let model = get_model("amazon-bedrock", "global.anthropic.claude-fable-5");

    let payload = capture_payload(&model, Some(options(Some(ThinkingLevel::XHigh), None)));

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "xhigh" }))
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn omits_display_for_govcloud_model_ids_on_non_adaptive_claude_thinking() {
    let base_model = get_model(
        "amazon-bedrock",
        "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
    );
    let model = with_id_and_name(
        base_model,
        "us-gov.anthropic.claude-sonnet-4-5-20250929-v1:0",
        "Claude Sonnet 4.5 (GovCloud)",
    );

    let payload = capture_payload(&model, None);

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "enabled", "budget_tokens": 16384 }))
    );
    assert_eq!(
        additional_field(&payload, "anthropic_beta"),
        Some(&json!(["interleaved-thinking-2025-05-14"]))
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn omits_display_for_govcloud_regions_on_adaptive_claude_thinking() {
    let base_model = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1");
    let model = with_id_and_name(
        base_model,
        "global.anthropic.claude-opus-4-8-v1",
        "Claude Opus 4.8 (Global)",
    );

    let payload = capture_payload(&model, Some(options(None, Some("us-gov-west-1"))));

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "high" }))
    );
    assert_eq!(additional_field(&payload, "anthropic_beta"), None);
}

struct BedrockResponse {
    stop_reason: &'static str,
    error_message: Option<&'static str>,
    output_usage: u32,
}

fn run_max_tokens_e2e(model: &Model) -> BedrockResponse {
    let context = json!({
        "systemPrompt": "You are a deterministic text generator. Follow the requested output format exactly.",
        "messages": [{
            "role": "user",
            "content": "Output exactly 5200 repetitions of the token alpha, separated by single spaces. Do not number them. Do not use markdown. Do not add any other text.",
            "timestamp": 0
        }]
    });
    let _ = (model, context, options(Some(ThinkingLevel::Low), None));
    panic!("{LIVE_BLOCKER}");
}

#[test]
#[ignore = "live Bedrock provider parity test requires AWS credentials and network calls"]
fn uses_model_max_tokens_cap_instead_of_bedrock_4096_token_default_for_adaptive_claude_models() {
    let mut model = get_model("amazon-bedrock", "global.anthropic.claude-sonnet-4-6");
    model.max_tokens = 6000;

    let response = run_max_tokens_e2e(&model);

    assert_ne!(
        response.stop_reason,
        "error",
        "{}",
        response.error_message.unwrap_or_default()
    );
    assert!(response.output_usage > 4096);
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn uses_adaptive_thinking_when_model_name_contains_model_name_but_arn_does_not() {
    let base_model = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1");
    let model = with_id_and_name(
        base_model,
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-profile",
        "Claude Opus 4.6",
    );

    let payload = capture_payload(&model, None);

    assert_eq!(
        additional_field(&payload, "thinking"),
        Some(&json!({ "type": "adaptive", "display": "summarized" }))
    );
    assert_eq!(
        additional_field(&payload, "output_config"),
        Some(&json!({ "effort": "high" }))
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn injects_cache_points_when_model_name_identifies_supported_claude_model() {
    let base_model = get_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1");
    let model = with_id_and_name(
        base_model,
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-profile",
        "Claude Sonnet 4.6",
    );

    let captured_payload = capture_payload(&model, None);

    assert_eq!(
        captured_payload
            .get("system")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(captured_payload.pointer("/system/1/cachePoint").is_some());

    let last_content = captured_payload
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.last())
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .and_then(|content| content.last());
    assert!(
        last_content
            .and_then(|content| content.get("cachePoint"))
            .is_some()
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: Bedrock request-payload construction/on_payload capture is not ported"]
fn falls_back_to_fixed_budget_thinking_for_non_adaptive_claude_via_model_name() {
    let base_model = get_model(
        "amazon-bedrock",
        "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
    );
    let model = with_id_and_name(
        base_model,
        "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-profile",
        "Claude Sonnet 4.5",
    );

    let payload = capture_payload(&model, None);

    let thinking = additional_field(&payload, "thinking")
        .and_then(Value::as_object)
        .expect("thinking field is present");
    assert_eq!(thinking.get("type"), Some(&json!("enabled")));
    assert!(
        thinking
            .get("budget_tokens")
            .and_then(Value::as_u64)
            .is_some()
    );
    assert_eq!(
        additional_field(&payload, "anthropic_beta"),
        Some(&json!(["interleaved-thinking-2025-05-14"]))
    );
}
