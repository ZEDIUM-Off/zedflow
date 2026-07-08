use zedflow_ai::api::lazy::{Context, Model, StopReason};
use zedflow_ai::api::simple_options::{CacheRetention, StreamOptions};
use zedflow_ai::compat::{complete, get_models, get_providers};
use zedflow_ai::env_api_keys::get_env_api_key;

const BLOCKER: &str = "PORT PLACEHOLDER: compat::get_providers/get_models still return placeholders, compat::Model omits AnthropicMessagesCompat.supportsLongCacheRetention and model cost metadata, and live provider calls are not allowed in this port task";

#[derive(Debug, Clone)]
struct AnthropicLongCacheRetentionE2ECase {
    name: String,
    provider: String,
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

fn get_anthropic_messages_models(provider: &str) -> Vec<Model> {
    get_models()
        .expect("compat::get_models should expose the generated catalog")
        .into_iter()
        .filter(|model| model.provider == provider && model.api == "anthropic-messages")
        .collect()
}

fn anthropic_messages_cases() -> Vec<AnthropicLongCacheRetentionE2ECase> {
    get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            let api_key = get_e2e_api_key(&provider);
            get_anthropic_messages_models(&provider)
                .into_iter()
                .map(move |model| AnthropicLongCacheRetentionE2ECase {
                    name: format!("{provider}/{}", model.id),
                    provider: provider.clone(),
                    model,
                    api_key: api_key.clone(),
                })
        })
        .collect()
}

fn get_probe_priority(model: &Model) -> i64 {
    let model_id = model.id.to_lowercase();
    let mut priority = 0;

    // PORT PLACEHOLDER: compat::Model does not expose model.cost yet; restore cost-based sorting
    // when the unified generated catalog is wired into compat::get_models.
    if model_id.contains("haiku") && (model_id.contains("4-5") || model_id.contains("4.5")) {
        priority -= 1000;
    } else if model_id.contains("sonnet") && (model_id.contains("4-") || model_id.contains("4.")) {
        priority -= 750;
    } else if model_id.contains("claude") && (model_id.contains("4-") || model_id.contains("4.")) {
        priority -= 500;
    }

    priority
}

fn select_one_case_per_provider(
    mut cases: Vec<AnthropicLongCacheRetentionE2ECase>,
) -> Vec<AnthropicLongCacheRetentionE2ECase> {
    cases.sort_by(|a, b| {
        a.provider
            .cmp(&b.provider)
            .then_with(|| get_probe_priority(&a.model).cmp(&get_probe_priority(&b.model)))
            .then_with(|| a.model.id.cmp(&b.model.id))
    });

    let mut selected = Vec::new();
    for test_case in cases {
        if selected
            .last()
            .is_none_or(|last: &AnthropicLongCacheRetentionE2ECase| {
                last.provider != test_case.provider
            })
        {
            selected.push(test_case);
        }
    }
    selected
}

fn with_long_cache_retention(_model: &Model) -> Result<Model, &'static str> {
    Err(BLOCKER)
}

fn expect_long_cache_retention_accepted(model: &Model, api_key: Option<&str>) {
    let response = complete(
        model,
        &Context,
        Some(StreamOptions {
            api_key: api_key.map(str::to_owned),
            cache_retention: Some(CacheRetention::Long),
            max_tokens: Some(128),
            ..StreamOptions::default()
        }),
    )
    .expect("long cache retention request should complete");

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

#[test]
#[ignore = "PORT PLACEHOLDER: compat::get_providers/get_models still return placeholders until the generated provider catalog is wired"]
fn covers_every_generated_anthropic_messages_model() {
    let mut actual = anthropic_messages_cases()
        .into_iter()
        .map(|test_case| test_case.name)
        .collect::<Vec<_>>();
    let mut expected = get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            get_anthropic_messages_models(&provider)
                .into_iter()
                .map(move |model| format!("{provider}/{}", model.id))
        })
        .collect::<Vec<_>>();

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
#[ignore = "live provider call skipped; compat catalog, Anthropic long-cache-retention compat override, thinkingEnabled option, and provider streaming remain PORT PLACEHOLDERs"]
fn forced_long_cache_retention_probe_accepts_long_cache_retention() {
    for test_case in select_one_case_per_provider(anthropic_messages_cases())
        .into_iter()
        .filter(|test_case| test_case.api_key.is_some())
    {
        let model = with_long_cache_retention(&test_case.model)
            .expect("model compat override should be applied before the provider request");
        expect_long_cache_retention_accepted(&model, test_case.api_key.as_deref());
    }
}
