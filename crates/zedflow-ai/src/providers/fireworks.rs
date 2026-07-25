//! Fireworks provider factory ported from Pi's `packages/ai/src/providers/fireworks.ts`.

use std::collections::HashMap;

use crate::error::Result;

use crate::models::{Provider, ProviderApi};
use crate::providers::fireworks_models::FireworksModel;
use crate::providers::static_catalog::{models_from_catalog, static_provider};
use crate::types::{
    AnthropicMessagesCompat, Model, ModelCompat, ModelThinkingLevel, OpenAICompletionsCompat,
};

/// Fireworks provider id used by Pi.
pub const FIREWORKS_PROVIDER_ID: &str = "fireworks";

/// Fireworks display name used by Pi.
pub const FIREWORKS_PROVIDER_NAME: &str = "Fireworks";

/// Fireworks API base URL used by Pi's provider factory.
pub const FIREWORKS_BASE_URL: &str = "https://api.fireworks.ai/inference";

/// Fireworks API-key auth prompt label used by Pi.
pub const FIREWORKS_API_KEY_AUTH_NAME: &str = "Fireworks API key";

/// Environment variables checked for Fireworks API-key auth, in Pi precedence order.
pub const FIREWORKS_API_KEY_ENV_VARS: &[&str] = &["FIREWORKS_API_KEY"];

/// Fireworks chat API ids registered by Pi.
pub const FIREWORKS_APIS: &[&str] = &["anthropic-messages", "openai-completions"];

fn apply_catalog_metadata(model: &mut Model, source: &FireworksModel) {
    model.compat = Some(match source.api {
        "anthropic-messages" => ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: source.compat.supports_eager_tool_input_streaming,
            supports_long_cache_retention: source.compat.supports_long_cache_retention,
            send_session_affinity_headers: source.compat.send_session_affinity_headers,
            supports_cache_control_on_tools: source.compat.supports_cache_control_on_tools,
            ..AnthropicMessagesCompat::default()
        }),
        _ => ModelCompat::OpenAICompletions(OpenAICompletionsCompat {
            supports_store: source.compat.supports_store,
            supports_developer_role: source.compat.supports_developer_role,
            ..OpenAICompletionsCompat::default()
        }),
    });
    model.thinking_level_map = source.thinking_level_map.map(|entries| {
        entries
            .iter()
            .map(|(level, value)| {
                let level = match *level {
                    "off" => ModelThinkingLevel::Off,
                    "minimal" => ModelThinkingLevel::Minimal,
                    "low" => ModelThinkingLevel::Low,
                    "medium" => ModelThinkingLevel::Medium,
                    "high" => ModelThinkingLevel::High,
                    _ => ModelThinkingLevel::XHigh,
                };
                (level, value.map(str::to_owned))
            })
            .collect()
    });
}

/// Creates the Fireworks provider from the static Rust model catalog.
pub fn fireworks_provider() -> Result<Provider> {
    let mut models = models_from_catalog(crate::providers::fireworks_models::FIREWORKS_MODELS);
    for model in &mut models {
        if let Some(source) = crate::providers::fireworks_models::FIREWORKS_MODELS
            .iter()
            .find(|source| source.id == model.id)
        {
            apply_catalog_metadata(model, source);
        }
    }
    let mut provider = static_provider(FIREWORKS_PROVIDER_ID, FIREWORKS_PROVIDER_NAME, models);
    provider.api = ProviderApi::ByApi(HashMap::from([
        (
            "anthropic-messages".to_owned(),
            crate::api::anthropic_messages::provider_streams(),
        ),
        (
            "openai-completions".to_owned(),
            crate::api::openai_completions::provider_streams(),
        ),
    ]));
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = fireworks_provider().expect("provider");
        assert_eq!(provider.id, FIREWORKS_PROVIDER_ID);
        assert_eq!(provider.name, FIREWORKS_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
