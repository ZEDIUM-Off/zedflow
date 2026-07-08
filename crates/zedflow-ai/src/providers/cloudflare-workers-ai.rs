//! Cloudflare Workers AI provider factory ported from Pi's `packages/ai/src/providers/cloudflare-workers-ai.ts`.

use std::sync::Arc;

use crate::models::{
    AssistantMessageEventStream, CreateProviderOptions, Model, Provider, StreamOptions,
    create_provider,
};
use crate::providers::cloudflare_workers_ai_models::CLOUDFLARE_WORKERS_AI_MODELS;

/// Cloudflare Workers AI provider id used by Pi.
pub const CLOUDFLARE_WORKERS_AI_PROVIDER_ID: &str = "cloudflare-workers-ai";

/// Cloudflare Workers AI display name used by Pi.
pub const CLOUDFLARE_WORKERS_AI_PROVIDER_NAME: &str = "Cloudflare Workers AI";

/// Cloudflare Workers AI stream API id used by Pi models.
pub const CLOUDFLARE_WORKERS_AI_API: &str = "openai-completions";

/// Cloudflare Workers AI API-key auth prompt label used by Pi.
pub const CLOUDFLARE_WORKERS_AI_API_KEY_AUTH_NAME: &str = "Cloudflare API key";

/// Creates Pi's Cloudflare Workers AI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/providers/cloudflare-auth.ts cloudflareWorkersAIAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi`.
/// Reason: the current Rust `Provider` shape has model catalog and stream hooks, but no auth/API fields.
/// Required behavior: `return createProvider({ id: "cloudflare-workers-ai", name: "Cloudflare Workers AI", auth: { apiKey: cloudflareWorkersAIAuth() }, models: Object.values(CLOUDFLARE_WORKERS_AI_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production streaming/auth use.
#[must_use]
pub fn cloudflare_workers_ai_provider() -> Provider {
    create_provider(CreateProviderOptions {
        id: CLOUDFLARE_WORKERS_AI_PROVIDER_ID.into(),
        name: Some(CLOUDFLARE_WORKERS_AI_PROVIDER_NAME.into()),
        models: CLOUDFLARE_WORKERS_AI_MODELS
            .iter()
            .map(|model| Model {
                provider: model.provider.to_owned(),
                id: model.id.to_owned(),
                api: model.api.to_owned(),
            })
            .collect(),
        refresh_models: None,
        stream: Arc::new(|_model: &Model, _options: Option<&StreamOptions>| {
            AssistantMessageEventStream::new()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_cloudflare_workers_ai_provider() {
        let provider = cloudflare_workers_ai_provider();

        assert_eq!(provider.id, CLOUDFLARE_WORKERS_AI_PROVIDER_ID);
        assert_eq!(provider.name, CLOUDFLARE_WORKERS_AI_PROVIDER_NAME);
        assert_eq!(
            provider.get_models().len(),
            CLOUDFLARE_WORKERS_AI_MODELS.len()
        );
        assert!(
            provider
                .get_models()
                .iter()
                .all(|model| model.provider == CLOUDFLARE_WORKERS_AI_PROVIDER_ID)
        );
        assert!(
            provider
                .get_models()
                .iter()
                .all(|model| model.api == CLOUDFLARE_WORKERS_AI_API)
        );
    }

    #[test]
    fn preserves_cloudflare_workers_ai_constants() {
        assert_eq!(CLOUDFLARE_WORKERS_AI_PROVIDER_ID, "cloudflare-workers-ai");
        assert_eq!(CLOUDFLARE_WORKERS_AI_PROVIDER_NAME, "Cloudflare Workers AI");
        assert_eq!(CLOUDFLARE_WORKERS_AI_API, "openai-completions");
        assert_eq!(
            CLOUDFLARE_WORKERS_AI_API_KEY_AUTH_NAME,
            "Cloudflare API key"
        );
    }
}
