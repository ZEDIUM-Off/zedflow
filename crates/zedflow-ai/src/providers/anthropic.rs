//! Anthropic provider factory ported from Pi's `packages/ai/src/providers/anthropic.ts`.

use zedflow_core::error::Result;

use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::static_provider;
use crate::types::{
    AnthropicMessagesCompat, Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel,
    ThinkingLevelMap,
};

/// Anthropic provider id used by Pi.
pub const ANTHROPIC_PROVIDER_ID: &str = "anthropic";

/// Anthropic display name used by Pi.
pub const ANTHROPIC_PROVIDER_NAME: &str = "Anthropic";

/// Anthropic API base URL used by Pi.
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

/// Anthropic API-key auth prompt label used by Pi.
pub const ANTHROPIC_API_KEY_AUTH_NAME: &str = "Anthropic API key";

/// Anthropic OAuth prompt label used by Pi.
pub const ANTHROPIC_OAUTH_NAME: &str = "Anthropic (Claude Pro/Max)";

/// Environment variables checked for Anthropic API-key auth, in Pi precedence order.
pub const ANTHROPIC_API_KEY_ENV_VARS: &[&str] = &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"];

/// Creates the anthropic provider from the static Rust model catalog.
pub fn anthropic_provider() -> Result<Provider> {
    let mut provider = static_provider(
        ANTHROPIC_PROVIDER_ID,
        ANTHROPIC_PROVIDER_NAME,
        registered_anthropic_models(),
    );
    provider.base_url = Some(ANTHROPIC_BASE_URL.to_owned());
    provider.api =
        ProviderApi::Single(crate::api::anthropic_messages_lazy::anthropic_messages_api());
    Ok(provider)
}

fn registered_anthropic_models() -> Vec<Model> {
    crate::providers::anthropic_models::ANTHROPIC_MODELS
        .iter()
        .map(|source| Model {
            id: source.id.to_owned(),
            name: source.name.to_owned(),
            api: source.api.to_owned(),
            provider: source.provider.to_owned(),
            base_url: source.base_url.to_owned(),
            reasoning: source.reasoning,
            thinking_level_map: source.thinking_level_map.map(|entries| {
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
            }),
            input: source
                .input
                .iter()
                .filter_map(|input| match *input {
                    "text" => Some(ModelInput::Text),
                    "image" => Some(ModelInput::Image),
                    _ => None,
                })
                .collect(),
            cost: ModelCost {
                input: source.cost.input,
                output: source.cost.output,
                cache_read: source.cost.cache_read,
                cache_write: source.cost.cache_write,
            },
            context_window: u64::from(source.context_window),
            max_tokens: u64::from(source.max_tokens),
            headers: None,
            compat: source.compat.map(|compat| {
                ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
                    force_adaptive_thinking: compat.force_adaptive_thinking,
                    supports_temperature: compat.supports_temperature,
                    ..AnthropicMessagesCompat::default()
                })
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, executor::block_on};

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = anthropic_provider().expect("provider");
        assert_eq!(provider.id, ANTHROPIC_PROVIDER_ID);
        assert_eq!(provider.name, ANTHROPIC_PROVIDER_NAME);
        assert_eq!(provider.base_url.as_deref(), Some(ANTHROPIC_BASE_URL));
        assert!(provider.auth.api_key.is_some());
        assert!(provider.auth.oauth.is_some());
        assert_eq!(
            ANTHROPIC_API_KEY_ENV_VARS,
            &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]
        );
        let models = provider.get_models();
        assert!(!models.is_empty());
        assert!(models.iter().all(|model| !model.base_url.is_empty()));
        assert!(models.iter().any(|model| model.cost.input > 0.0));
    }

    #[test]
    fn registered_api_uses_real_anthropic_transport() {
        let provider = anthropic_provider().expect("provider");
        let model = provider.get_models().into_iter().next().expect("model");
        let mut stream = provider.stream(&model, &crate::types::Context::default(), None);
        let event = block_on(stream.next()).expect("terminal auth error");
        let crate::types::AssistantMessageEvent::Error { error, .. } = event else {
            panic!("expected auth error");
        };
        assert!(error.error_message.as_deref().is_some_and(|message| {
            message.contains("No API key") || message.contains("no API key")
        }));
        assert!(
            !error
                .error_message
                .as_deref()
                .unwrap_or_default()
                .contains("transport is not implemented")
        );
        assert_eq!(block_on(stream.next()), None);
    }
}
