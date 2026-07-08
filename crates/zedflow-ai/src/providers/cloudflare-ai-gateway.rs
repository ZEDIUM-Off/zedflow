//! Cloudflare AI Gateway provider ported from Pi's `packages/ai/src/providers/cloudflare-ai-gateway.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Cloudflare AI Gateway provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/providers/cloudflare-ai-gateway.models.ts CLOUDFLARE_AI_GATEWAY_MODELS, references/pi/packages/ai/src/providers/cloudflare-auth.ts cloudflareAIGatewayAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "cloudflare-ai-gateway", name: "Cloudflare AI Gateway", auth: { apiKey: cloudflareAIGatewayAuth() }, models: Object.values(CLOUDFLARE_AI_GATEWAY_MODELS), api: { "anthropic-messages": anthropicMessagesApi(), "openai-completions": openAICompletionsApi(), "openai-responses": openAIResponsesApi() } })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the Cloudflare AI Gateway model catalog, auth resolver,
/// and stream API provider wiring are available in Rust.
pub fn cloudflare_ai_gateway_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/providers/cloudflare-ai-gateway.models.ts CLOUDFLARE_AI_GATEWAY_MODELS, references/pi/packages/ai/src/providers/cloudflare-auth.ts cloudflareAIGatewayAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi",
        "return createProvider({ id: \"cloudflare-ai-gateway\", name: \"Cloudflare AI Gateway\", auth: { apiKey: cloudflareAIGatewayAuth() }, models: Object.values(CLOUDFLARE_AI_GATEWAY_MODELS), api: { \"anthropic-messages\": anthropicMessagesApi(), \"openai-completions\": openAICompletionsApi(), \"openai-responses\": openAIResponsesApi() } })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_cloudflare_ai_gateway_provider_blocker() {
        let err = cloudflare_ai_gateway_provider()
            .expect_err("provider creation is intentionally blocked");
        match err {
            Error::PortPlaceholder(placeholder) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("cloudflareAIGatewayAuth")
                );
                assert!(placeholder.required_behavior().contains("openai-responses"));
            }
            _ => panic!("unexpected provider error: {err:?}"),
        }
    }

    #[test]
    fn preserves_cloudflare_ai_gateway_constants() {
        assert_eq!(CLOUDFLARE_AI_GATEWAY_PROVIDER_ID, "cloudflare-ai-gateway");
        assert_eq!(CLOUDFLARE_AI_GATEWAY_PROVIDER_NAME, "Cloudflare AI Gateway");
        assert_eq!(CLOUDFLARE_API_KEY_ENV, "CLOUDFLARE_API_KEY");
        assert_eq!(CLOUDFLARE_ACCOUNT_ID_ENV, "CLOUDFLARE_ACCOUNT_ID");
        assert_eq!(CLOUDFLARE_GATEWAY_ID_ENV, "CLOUDFLARE_GATEWAY_ID");
        assert_eq!(
            CLOUDFLARE_AI_GATEWAY_AUTHORIZATION_HEADER,
            "cf-aig-authorization"
        );
        assert_eq!(
            CLOUDFLARE_AI_GATEWAY_APIS,
            &[
                "anthropic-messages",
                "openai-completions",
                "openai-responses"
            ]
        );
    }
}
