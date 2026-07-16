//! GitHub Copilot provider factory ported from Pi's `packages/ai/src/providers/github-copilot.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use zedflow_core::error::Result;

use crate::api::github_copilot_headers::build_copilot_dynamic_headers_for_context;
use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::{models_from_catalog, static_provider};
use crate::types::{
    AnthropicMessagesCompat, Context, Model, ModelCompat, ModelThinkingLevel, ProviderHeaders,
    ProviderStreams, StreamOptions, ThinkingLevelMap,
};

/// GitHub Copilot provider id used by Pi.
pub const GITHUB_COPILOT_PROVIDER_ID: &str = "github-copilot";

/// GitHub Copilot display name used by Pi.
pub const GITHUB_COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";

/// GitHub Copilot API base URL used by Pi.
pub const GITHUB_COPILOT_BASE_URL: &str = "https://api.individual.githubcopilot.com";

/// GitHub Copilot API-key auth prompt label used by Pi.
pub const GITHUB_COPILOT_API_KEY_AUTH_NAME: &str = "GitHub Copilot token";

/// GitHub Copilot OAuth prompt label used by Pi.
pub const GITHUB_COPILOT_OAUTH_NAME: &str = "GitHub Copilot";

/// Environment variables checked for GitHub Copilot API-key auth, in Pi precedence order.
pub const GITHUB_COPILOT_API_KEY_ENV_VARS: &[&str] = &["COPILOT_GITHUB_TOKEN"];

/// API stream ids registered by Pi for GitHub Copilot.
pub const GITHUB_COPILOT_APIS: &[&str] = &[
    "anthropic-messages",
    "openai-completions",
    "openai-responses",
];

/// Creates the GitHub Copilot provider with Pi's mixed-API dispatch.
pub fn github_copilot_provider() -> Result<Provider> {
    let mut provider = static_provider(
        GITHUB_COPILOT_PROVIDER_ID,
        GITHUB_COPILOT_PROVIDER_NAME,
        registered_models(),
    );
    provider.base_url = Some(GITHUB_COPILOT_BASE_URL.to_owned());
    provider.api = ProviderApi::ByApi(HashMap::from([
        ("anthropic-messages".to_owned(), copilot_anthropic_streams()),
        (
            "openai-completions".to_owned(),
            crate::api::openai_completions_lazy::open_ai_completions_api(),
        ),
        (
            "openai-responses".to_owned(),
            crate::api::openai_responses_lazy::open_ai_responses_api(),
        ),
    ]));
    Ok(provider)
}

fn registered_models() -> Vec<Model> {
    let mut models =
        models_from_catalog(crate::providers::github_copilot_models::GITHUB_COPILOT_MODELS);
    for (model, source) in models
        .iter_mut()
        .zip(crate::providers::github_copilot_models::GITHUB_COPILOT_MODELS)
    {
        model.headers = Some(
            source
                .headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        );
        model.thinking_level_map = source.thinking_level_map.map(|entries| {
            entries
                .iter()
                .filter_map(|(level, value)| {
                    let level = match *level {
                        "off" => ModelThinkingLevel::Off,
                        "minimal" => ModelThinkingLevel::Minimal,
                        "low" => ModelThinkingLevel::Low,
                        "medium" => ModelThinkingLevel::Medium,
                        "high" => ModelThinkingLevel::High,
                        "xhigh" => ModelThinkingLevel::XHigh,
                        _ => return None,
                    };
                    Some((level, value.map(str::to_owned)))
                })
                .collect::<ThinkingLevelMap>()
        });
        if model.api == "anthropic-messages" {
            model.compat = source.compat.map(|compat| {
                ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
                    supports_eager_tool_input_streaming: compat.supports_eager_tool_input_streaming,
                    supports_temperature: compat.supports_temperature,
                    force_adaptive_thinking: compat.force_adaptive_thinking,
                    ..AnthropicMessagesCompat::default()
                })
            });
        }
    }
    models
}

fn copilot_anthropic_streams() -> ProviderStreams {
    let anthropic = crate::api::anthropic_messages_lazy::anthropic_messages_api();
    let simple_anthropic = anthropic.clone();
    ProviderStreams {
        stream: Arc::new(move |model, context, options| {
            let options = copilot_stream_options(context, options.cloned().unwrap_or_default());
            (anthropic.stream)(model, context, Some(&options))
        }),
        stream_simple: Arc::new(move |model, context, options| {
            let mut options = options.cloned().unwrap_or_default();
            options.stream = copilot_stream_options(context, options.stream);
            (simple_anthropic.stream_simple)(model, context, Some(&options))
        }),
    }
}

fn copilot_stream_options(context: &Context, mut options: StreamOptions) -> StreamOptions {
    let mut headers = copilot_headers(context);
    if let Some(option_headers) = options.headers.take() {
        headers.extend(option_headers);
    }
    options.headers = Some(headers);
    options
}

fn copilot_headers(context: &Context) -> ProviderHeaders {
    build_copilot_dynamic_headers_for_context(&context.messages)
        .into_iter()
        .map(|(name, value)| (name, Some(value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_with_exact_mixed_api_registration() {
        let provider = github_copilot_provider().expect("provider");
        assert_eq!(provider.id, GITHUB_COPILOT_PROVIDER_ID);
        assert_eq!(provider.name, GITHUB_COPILOT_PROVIDER_NAME);
        assert_eq!(provider.base_url.as_deref(), Some(GITHUB_COPILOT_BASE_URL));
        let ProviderApi::ByApi(apis) = &provider.api else {
            panic!("Copilot must dispatch by model API");
        };
        let mut actual = apis.keys().map(String::as_str).collect::<Vec<_>>();
        actual.sort_unstable();
        let mut expected = GITHUB_COPILOT_APIS.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected);
        assert!(provider.get_models().iter().any(|model| {
            model.api == "anthropic-messages"
                && model.headers.as_ref().is_some_and(|headers| {
                    headers.get("Copilot-Integration-Id").map(String::as_str) == Some("vscode-chat")
                })
        }));
    }
}
