use serde_json::{Value, json};

const PAYLOAD_BLOCKER: &str = "PORT PLACEHOLDER: anthropic request-payload construction, compat::get_model, and stream_simple on_payload capture are not ported yet; keep ignored until the real payload path exists.";
const LIVE_BLOCKER: &str = "live Anthropic API parity test intentionally ignored: requires ANTHROPIC_API_KEY and provider network calls.";

#[derive(Debug, Clone, Copy)]
struct Model {
    id: &'static str,
}

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

fn get_model(_provider: &str, id: &'static str) -> Model {
    Model { id }
}

fn make_payload_capture_context() -> Value {
    json!({
        "messages": [{ "role": "user", "content": "Hello", "timestamp": 0 }]
    })
}

fn capture_payload(model: Model, options: Option<SimpleStreamOptions>) -> Value {
    let _ = (
        model.id,
        options.map(|options| options.reasoning),
        make_payload_capture_context(),
    );
    panic!("{PAYLOAD_BLOCKER}");
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
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
fn sends_thinking_type_disabled_for_budget_based_reasoning_models_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-sonnet-4-5"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
fn sends_thinking_type_disabled_for_adaptive_reasoning_models_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-opus-4-6"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
fn sends_thinking_type_disabled_for_claude_opus_4_8_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-opus-4-8"), None);

    assert_eq!(
        payload.get("thinking"),
        Some(&json!({ "type": "disabled" }))
    );
    assert_eq!(payload.get("output_config"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
fn omits_thinking_type_disabled_for_claude_fable_5_when_thinking_is_off() {
    let payload = capture_payload(get_model("anthropic", "claude-fable-5"), None);

    assert_eq!(payload.get("thinking"), None);
    assert_eq!(payload.get("output_config"), None);
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
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
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
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
#[ignore = "PORT PLACEHOLDER: anthropic request-payload construction/on_payload capture is not ported"]
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
#[ignore = "live Anthropic API parity test skipped: no live provider calls in P1.T2"]
fn disables_thinking_for_claude_reasoning_models() {
    let result = run_without_reasoning(get_model("anthropic", "claude-sonnet-4-5"));

    assert_eq!(result.thinking_event_count, 0);
    assert_eq!(result.thinking_char_count, 0);
    assert!(!result.content_types.iter().any(|kind| kind == "thinking"));
    assert!(count_pongs(&result.text) >= 35);
}
