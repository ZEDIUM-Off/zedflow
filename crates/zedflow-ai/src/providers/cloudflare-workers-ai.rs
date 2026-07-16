//! Cloudflare Workers AI provider factory ported from Pi's `packages/ai/src/providers/cloudflare-workers-ai.ts`.

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Cloudflare Workers AI provider id used by Pi.
pub const CLOUDFLARE_WORKERS_AI_PROVIDER_ID: &str = "cloudflare-workers-ai";

/// Cloudflare Workers AI display name used by Pi.
pub const CLOUDFLARE_WORKERS_AI_PROVIDER_NAME: &str = "Cloudflare Workers AI";

/// Cloudflare Workers AI stream API id used by Pi models.
pub const CLOUDFLARE_WORKERS_AI_API: &str = "openai-completions";

/// Cloudflare Workers AI API-key auth prompt label used by Pi.
pub const CLOUDFLARE_WORKERS_AI_API_KEY_AUTH_NAME: &str = "Cloudflare API key";

/// Creates the cloudflare-workers-ai provider from the static Rust model catalog.
#[must_use]
pub fn cloudflare_workers_ai_provider() -> Provider {
    static_provider(
        CLOUDFLARE_WORKERS_AI_PROVIDER_ID,
        CLOUDFLARE_WORKERS_AI_PROVIDER_NAME,
        models_from_catalog(
            crate::providers::cloudflare_workers_ai_models::CLOUDFLARE_WORKERS_AI_MODELS,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = cloudflare_workers_ai_provider();
        assert_eq!(provider.id, CLOUDFLARE_WORKERS_AI_PROVIDER_ID);
        assert_eq!(provider.name, CLOUDFLARE_WORKERS_AI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
