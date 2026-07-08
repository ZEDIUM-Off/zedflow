use regex::Regex;

const LIVE_BLOCKER: &str = "live provider parity test intentionally ignored: requires provider credentials and network calls; P1.T2 forbids live provider calls and Rust stream/get_model paths are still port placeholders.";

#[derive(Debug, Clone, Copy)]
struct Model {
    provider: &'static str,
    id: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct SimpleOptionsWithExtras {
    max_tokens: Option<u32>,
    temperature: Option<f64>,
    api_key: Option<&'static str>,
    project: Option<&'static str>,
    location: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RunResult {
    thinking_event_count: usize,
    thinking_char_count: usize,
    text: String,
    output_tokens: u32,
    content_types: Vec<&'static str>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct DisableExpectations {
    request_options: Option<SimpleOptionsWithExtras>,
    min_pongs: Option<usize>,
    max_output_tokens: Option<u32>,
}

fn get_model(provider: &'static str, id: &'static str) -> Model {
    Model { provider, id }
}

fn make_context() -> (&'static str, &'static str) {
    (
        "You are a precise assistant. Follow the requested output format exactly.",
        "Before replying, carefully solve 36863 * 5279 internally. Then reply with the word pong repeated exactly 40 times, separated by single spaces. Do not add any other text.",
    )
}

fn count_pongs(text: &str) -> usize {
    Regex::new(r"(?i)\bpong\b")
        .expect("hard-coded pong regex should compile")
        .find_iter(text)
        .count()
}

fn run_without_reasoning(model: Model, options: Option<SimpleOptionsWithExtras>) -> RunResult {
    let _ = (model.provider, model.id, options, make_context());
    panic!("{LIVE_BLOCKER}");
}

fn expect_thinking_disabled_e2e(model: Model, expectations: DisableExpectations) {
    let result = run_without_reasoning(model, expectations.request_options);

    assert_eq!(result.thinking_event_count, 0);
    assert_eq!(result.thinking_char_count, 0);
    assert!(!result.content_types.iter().any(|kind| *kind == "thinking"));
    assert!(count_pongs(&result.text) >= expectations.min_pongs.unwrap_or(35));
    if let Some(max_output_tokens) = expectations.max_output_tokens {
        assert!(result.output_tokens < max_output_tokens);
    }
}

#[test]
#[ignore = "live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls"]
fn disables_thinking_for_anthropic_budget_based_reasoning_models() {
    expect_thinking_disabled_e2e(
        get_model("anthropic", "claude-sonnet-4-5"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                max_tokens: Some(320),
                temperature: Some(0.0),
                ..SimpleOptionsWithExtras::default()
            }),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls"]
fn disables_thinking_for_anthropic_adaptive_reasoning_models() {
    expect_thinking_disabled_e2e(
        get_model("anthropic", "claude-sonnet-4-6"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                max_tokens: Some(320),
                temperature: Some(0.0),
                ..SimpleOptionsWithExtras::default()
            }),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls"]
fn disables_thinking_for_gemini_2_5() {
    expect_thinking_disabled_e2e(
        get_model("google", "gemini-2.5-flash"),
        DisableExpectations::default(),
    );
}

#[test]
#[ignore = "live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls"]
fn disables_thinking_for_gemini_3_x() {
    expect_thinking_disabled_e2e(
        get_model("google", "gemini-3-flash-preview"),
        DisableExpectations::default(),
    );
}

#[test]
#[ignore = "live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls"]
fn does_not_error_when_thinking_is_off_for_gemini_3_1_pro() {
    expect_thinking_disabled_e2e(
        get_model("google", "gemini-3.1-pro-preview"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                max_tokens: Some(512),
                ..SimpleOptionsWithExtras::default()
            }),
            min_pongs: Some(20),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live Google Vertex parity test skipped: requires GOOGLE_CLOUD_API_KEY or GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_LOCATION and provider network calls"]
fn disables_thinking_for_vertex_gemini_2_5() {
    expect_thinking_disabled_e2e(
        get_model("google-vertex", "gemini-2.5-flash"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                api_key: Some("<GOOGLE_CLOUD_API_KEY>"),
                project: Some("<GOOGLE_CLOUD_PROJECT>"),
                location: Some("<GOOGLE_CLOUD_LOCATION>"),
                ..SimpleOptionsWithExtras::default()
            }),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live Google Vertex parity test skipped: requires GOOGLE_CLOUD_API_KEY or GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_LOCATION and provider network calls"]
fn disables_thinking_for_vertex_gemini_3_x() {
    expect_thinking_disabled_e2e(
        get_model("google-vertex", "gemini-3-flash-preview"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                api_key: Some("<GOOGLE_CLOUD_API_KEY>"),
                project: Some("<GOOGLE_CLOUD_PROJECT>"),
                location: Some("<GOOGLE_CLOUD_LOCATION>"),
                ..SimpleOptionsWithExtras::default()
            }),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live OpenAI API parity test skipped: requires OPENAI_API_KEY and provider network calls"]
fn disables_thinking_for_openai_responses_reasoning_models() {
    expect_thinking_disabled_e2e(
        get_model("openai", "gpt-5.4-mini"),
        DisableExpectations {
            request_options: Some(SimpleOptionsWithExtras {
                temperature: None,
                ..SimpleOptionsWithExtras::default()
            }),
            ..DisableExpectations::default()
        },
    );
}

#[test]
#[ignore = "live OpenRouter API parity test skipped: requires OPENROUTER_API_KEY and provider network calls"]
fn disables_thinking_for_openrouter_qwen_3_5_reasoning_models() {
    expect_thinking_disabled_e2e(
        get_model("openrouter", "qwen/qwen3.5-plus-02-15"),
        DisableExpectations {
            max_output_tokens: Some(100),
            ..DisableExpectations::default()
        },
    );
}
