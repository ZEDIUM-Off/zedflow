use zedflow_ai::api::lazy::{Context, Model, StopReason};
use zedflow_ai::api::simple_options::StreamOptions;
use zedflow_ai::compat::{complete, get_models, get_providers};
use zedflow_ai::env_api_keys::get_env_api_key;

#[derive(Debug, Clone)]
struct AnthropicEagerE2ECase {
    name: String,
    model: Model,
    api_key: Option<String>,
}

fn get_e2e_api_key(provider: &str) -> Option<String> {
    if provider == "github-copilot" {
        // PORT PLACEHOLDER: references/pi/packages/ai/test/oauth.ts resolveApiKey("github-copilot").
        return None;
    }

    get_env_api_key(provider, None)
}

fn anthropic_message_model_names() -> Vec<String> {
    let models = get_models().expect("compat::get_models should expose the generated catalog");
    get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            models
                .iter()
                .filter(|model| model.provider == provider && model.api == "anthropic-messages")
                .map(|model| format!("{provider}/{}", model.id))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn anthropic_messages_cases() -> Vec<AnthropicEagerE2ECase> {
    let models = get_models().expect("compat::get_models should expose the generated catalog");
    get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            let api_key = get_e2e_api_key(&provider);
            models
                .iter()
                .filter(|model| model.provider == provider && model.api == "anthropic-messages")
                .map(|model| AnthropicEagerE2ECase {
                    name: format!("{provider}/{}", model.id),
                    model: model.clone(),
                    api_key: api_key.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn expect_tool_enabled_request_accepted(model: &Model, api_key: Option<&str>) {
    let response = complete(
        model,
        &Context,
        Some(StreamOptions {
            api_key: api_key.map(str::to_owned),
            max_tokens: Some(128),
            ..StreamOptions::default()
        }),
    )
    .expect("tool-enabled request should complete");

    assert!(
        response.error_message.is_none(),
        "{:?}",
        response.error_message
    );
    assert_ne!(
        response.stop_reason,
        StopReason::Error,
        "{:?}",
        response.error_message
    );
}

fn with_eager_tool_input_streaming(_model: &Model) -> Result<Model, &'static str> {
    Err(
        "PORT PLACEHOLDER: compat Model does not expose AnthropicMessagesCompat.supportsEagerToolInputStreaming yet",
    )
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat::get_providers/get_models still return placeholders until the generated provider catalog is wired"]
fn covers_every_generated_anthropic_messages_model() {
    let mut actual = anthropic_messages_cases()
        .into_iter()
        .map(|test_case| test_case.name)
        .collect::<Vec<_>>();
    let mut expected = anthropic_message_model_names();

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
#[ignore = "live provider call skipped; compat catalog, Context tools, and provider streaming remain PORT PLACEHOLDERs"]
fn generated_compat_settings_accept_configured_tool_streaming() {
    let Some(test_case) = anthropic_messages_cases()
        .into_iter()
        .find(|test_case| test_case.api_key.is_some())
    else {
        return;
    };

    expect_tool_enabled_request_accepted(&test_case.model, test_case.api_key.as_deref());
}

#[test]
#[ignore = "live provider call skipped; compat Model eager-tool-input metadata remains a PORT PLACEHOLDER"]
fn forced_eager_input_streaming_probe_accepts_forced_eager_input_streaming() {
    let Some(test_case) = anthropic_messages_cases()
        .into_iter()
        .find(|test_case| test_case.api_key.is_some())
    else {
        return;
    };
    let model = with_eager_tool_input_streaming(&test_case.model)
        .expect("model compat override should be applied before the provider request");

    expect_tool_enabled_request_accepted(&model, test_case.api_key.as_deref());
}
