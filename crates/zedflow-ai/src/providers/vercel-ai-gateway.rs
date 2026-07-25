//! Vercel AI Gateway provider factory ported from Pi's `packages/ai/src/providers/vercel-ai-gateway.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Vercel AI Gateway provider id used by Pi.
pub const VERCEL_AI_GATEWAY_PROVIDER_ID: &str = "vercel-ai-gateway";

/// Vercel AI Gateway display name used by Pi.
pub const VERCEL_AI_GATEWAY_PROVIDER_NAME: &str = "Vercel AI Gateway";

/// Vercel AI Gateway API base URL used by Pi.
pub const VERCEL_AI_GATEWAY_BASE_URL: &str = "https://ai-gateway.vercel.sh";

/// Vercel AI Gateway stream API id used by Pi models.
pub const VERCEL_AI_GATEWAY_API: &str = "anthropic-messages";

/// Vercel AI Gateway API-key auth prompt label used by Pi.
pub const VERCEL_AI_GATEWAY_API_KEY_AUTH_NAME: &str = "Vercel AI Gateway API key";

/// Environment variables checked for Vercel AI Gateway API-key auth, in Pi precedence order.
pub const VERCEL_AI_GATEWAY_API_KEY_ENV_VARS: &[&str] = &["AI_GATEWAY_API_KEY"];

/// Creates the vercel-ai-gateway provider from the static Rust model catalog.
pub fn vercel_ai_gateway_provider() -> Result<Provider> {
    let provider = static_provider(
        VERCEL_AI_GATEWAY_PROVIDER_ID,
        VERCEL_AI_GATEWAY_PROVIDER_NAME,
        models_from_catalog(crate::providers::vercel_ai_gateway_models::VERCEL_AI_GATEWAY_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = vercel_ai_gateway_provider().expect("provider");
        assert_eq!(provider.id, VERCEL_AI_GATEWAY_PROVIDER_ID);
        assert_eq!(provider.name, VERCEL_AI_GATEWAY_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
