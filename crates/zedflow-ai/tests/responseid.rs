//! Port of Pi `packages/ai/test/responseid.test.ts`.
//!
//! The source file is a live provider E2E suite gated by credentials/OAuth tokens. P1.T2 forbids
//! live provider calls, and the Rust compat catalog/dispatch path plus response-id plumbing are
//! still port placeholders, so each parity case is represented as an ignored test.

use serde_json::Value;

const BLOCKER: &str = "live responseId E2E test skipped; requires provider credentials/OAuth tokens plus completed compat::get_model/complete and provider response_id plumbing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Model {
    provider: &'static str,
    id: &'static str,
    api: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StreamOptionsWithExtras {
    api_key: Option<&'static str>,
    project: Option<&'static str>,
    location: Option<&'static str>,
    azure_deployment_name: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Context {
    system_prompt: &'static str,
    user_message: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct Response {
    stop_reason: &'static str,
    error_message: Option<String>,
    response_id: Value,
}

fn get_model(provider: &'static str, id: &'static str) -> Model {
    let api = match provider {
        "anthropic" => "anthropic-messages",
        "azure-openai-responses" => "azure-openai-responses",
        "github-copilot" => "github-copilot",
        "google" => "google-generative-ai",
        "google-vertex" => "google-vertex",
        "mistral" => "mistral-conversations",
        "openai" => "openai-responses",
        "openai-codex" => "openai-codex-responses",
        _ => provider,
    };

    Model { provider, id, api }
}

fn openai_completions_model() -> Model {
    Model {
        api: "openai-completions",
        ..get_model("openai", "gpt-4o-mini")
    }
}

fn make_context() -> Context {
    Context {
        system_prompt: "You are a helpful assistant. Be concise.",
        user_message: "Reply with exactly: response id test",
    }
}

fn complete(
    model: Model,
    context: &Context,
    options: StreamOptionsWithExtras,
) -> Result<Response, &'static str> {
    let _source_fixture = (model, context, options);
    Err(BLOCKER)
}

fn expect_response_id(model: Model, options: StreamOptionsWithExtras) {
    let context = make_context();
    let response =
        complete(model, &context, options).expect("live responseId request should complete");

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(
        response
            .response_id
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert!(response.response_id.is_string());
}

#[test]
#[ignore = "live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls"]
fn google_provider_exposes_response_id() {
    expect_response_id(
        get_model("google", "gemini-2.5-flash"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live Google Vertex ADC parity test skipped: requires GOOGLE_CLOUD_PROJECT/GOOGLE_CLOUD_LOCATION and provider network calls"]
fn google_vertex_provider_exposes_response_id_with_adc() {
    expect_response_id(
        get_model("google-vertex", "gemini-3-flash-preview"),
        StreamOptionsWithExtras {
            project: Some("<GOOGLE_CLOUD_PROJECT>"),
            location: Some("<GOOGLE_CLOUD_LOCATION>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live Google Vertex API key parity test skipped: requires GOOGLE_CLOUD_API_KEY and provider network calls"]
fn google_vertex_provider_exposes_response_id_with_api_key() {
    expect_response_id(
        get_model("google-vertex", "gemini-3-flash-preview"),
        StreamOptionsWithExtras {
            api_key: Some("<GOOGLE_CLOUD_API_KEY>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live OpenAI Completions parity test skipped: requires OPENAI_API_KEY and provider network calls"]
fn openai_completions_provider_exposes_response_id() {
    expect_response_id(
        openai_completions_model(),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live OpenAI Responses parity test skipped: requires OPENAI_API_KEY and provider network calls"]
fn openai_responses_provider_exposes_response_id() {
    expect_response_id(
        get_model("openai", "gpt-5-mini"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls"]
fn anthropic_provider_exposes_response_id() {
    expect_response_id(
        get_model("anthropic", "claude-sonnet-4-5"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live Azure OpenAI Responses parity test skipped: requires Azure OpenAI credentials and provider network calls"]
fn azure_openai_responses_provider_exposes_response_id() {
    expect_response_id(
        get_model("azure-openai-responses", "gpt-4o-mini"),
        StreamOptionsWithExtras {
            azure_deployment_name: Some("<AZURE_OPENAI_DEPLOYMENT_NAME>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live Mistral API parity test skipped: requires MISTRAL_API_KEY and provider network calls"]
fn mistral_provider_exposes_response_id() {
    expect_response_id(
        get_model("mistral", "devstral-medium-latest"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live GitHub Copilot OpenAI-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls"]
fn github_copilot_openai_path_exposes_response_id() {
    expect_response_id(
        get_model("github-copilot", "gpt-5.3-codex"),
        StreamOptionsWithExtras {
            api_key: Some("<github-copilot OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live GitHub Copilot Anthropic-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls"]
fn github_copilot_anthropic_path_exposes_response_id() {
    expect_response_id(
        get_model("github-copilot", "claude-sonnet-4.6"),
        StreamOptionsWithExtras {
            api_key: Some("<github-copilot OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live OpenAI Codex parity test skipped: requires resolved openai-codex OAuth token and provider network calls"]
fn openai_codex_provider_exposes_response_id() {
    expect_response_id(
        get_model("openai-codex", "gpt-5.5"),
        StreamOptionsWithExtras {
            api_key: Some("<openai-codex OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}
