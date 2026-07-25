//! Cloudflare AI Gateway provider ported from Pi's `packages/ai/src/providers/cloudflare-ai-gateway.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Cloudflare AI Gateway provider id used by Pi.
pub const CLOUDFLARE_AI_GATEWAY_PROVIDER_ID: &str = "cloudflare-ai-gateway";

/// Cloudflare AI Gateway display name used by Pi.
pub const CLOUDFLARE_AI_GATEWAY_PROVIDER_NAME: &str = "Cloudflare AI Gateway";

/// Cloudflare API-key auth prompt label used by Pi.
pub const CLOUDFLARE_AI_GATEWAY_API_KEY_AUTH_NAME: &str = "Cloudflare API key";

/// Environment variable used for the Cloudflare bearer key.
pub const CLOUDFLARE_API_KEY_ENV: &str = "CLOUDFLARE_API_KEY";

/// Environment variable used for the Cloudflare account id.
pub const CLOUDFLARE_ACCOUNT_ID_ENV: &str = "CLOUDFLARE_ACCOUNT_ID";

/// Environment variable used for the Cloudflare AI Gateway id.
pub const CLOUDFLARE_GATEWAY_ID_ENV: &str = "CLOUDFLARE_GATEWAY_ID";

/// Header name used by Cloudflare AI Gateway for the bearer key.
pub const CLOUDFLARE_AI_GATEWAY_AUTHORIZATION_HEADER: &str = "cf-aig-authorization";

/// Headers cleared by Pi when resolving Cloudflare AI Gateway auth.
pub const CLOUDFLARE_AI_GATEWAY_CLEARED_HEADERS: &[&str] = &["Authorization", "x-api-key"];

/// API stream ids configured by Pi for Cloudflare AI Gateway models.
pub const CLOUDFLARE_AI_GATEWAY_APIS: &[&str] = &[
    "anthropic-messages",
    "openai-completions",
    "openai-responses",
];

/// Anthropic base URL template used by Cloudflare AI Gateway models.
pub const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL: &str = "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic";

/// OpenAI base URL template used by Cloudflare AI Gateway models.
pub const CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai";

/// Creates the cloudflare-ai-gateway provider from the static Rust model catalog.
pub fn cloudflare_ai_gateway_provider() -> Result<Provider> {
    let provider = static_provider(
        CLOUDFLARE_AI_GATEWAY_PROVIDER_ID,
        CLOUDFLARE_AI_GATEWAY_PROVIDER_NAME,
        crate::providers::cloudflare_ai_gateway_models::cloudflare_ai_gateway_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = cloudflare_ai_gateway_provider().expect("provider");
        assert_eq!(provider.id, CLOUDFLARE_AI_GATEWAY_PROVIDER_ID);
        assert_eq!(provider.name, CLOUDFLARE_AI_GATEWAY_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
