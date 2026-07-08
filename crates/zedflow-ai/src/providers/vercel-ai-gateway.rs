//! Vercel AI Gateway provider factory ported from Pi's `packages/ai/src/providers/vercel-ai-gateway.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Vercel AI Gateway provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/vercel-ai-gateway.models.ts VERCEL_AI_GATEWAY_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "vercel-ai-gateway", name: "Vercel AI Gateway", baseUrl: "https://ai-gateway.vercel.sh", auth: { apiKey: envApiKeyAuth("Vercel AI Gateway API key", ["AI_GATEWAY_API_KEY"]) }, models: Object.values(VERCEL_AI_GATEWAY_MODELS), api: anthropicMessagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Vercel AI Gateway model catalog are available in Rust.
#[must_use]
pub fn vercel_ai_gateway_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/vercel-ai-gateway.models.ts VERCEL_AI_GATEWAY_MODELS",
        "return createProvider({ id: \"vercel-ai-gateway\", name: \"Vercel AI Gateway\", baseUrl: \"https://ai-gateway.vercel.sh\", auth: { apiKey: envApiKeyAuth(\"Vercel AI Gateway API key\", [\"AI_GATEWAY_API_KEY\"]) }, models: Object.values(VERCEL_AI_GATEWAY_MODELS), api: anthropicMessagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_vercel_ai_gateway_provider_blocker() {
        match vercel_ai_gateway_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("VERCEL_AI_GATEWAY_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("anthropicMessagesApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("AI_GATEWAY_API_KEY")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_vercel_ai_gateway_provider_constants() {
        assert_eq!(VERCEL_AI_GATEWAY_PROVIDER_ID, "vercel-ai-gateway");
        assert_eq!(VERCEL_AI_GATEWAY_PROVIDER_NAME, "Vercel AI Gateway");
        assert_eq!(VERCEL_AI_GATEWAY_BASE_URL, "https://ai-gateway.vercel.sh");
        assert_eq!(VERCEL_AI_GATEWAY_API, "anthropic-messages");
        assert_eq!(
            VERCEL_AI_GATEWAY_API_KEY_AUTH_NAME,
            "Vercel AI Gateway API key"
        );
        assert_eq!(VERCEL_AI_GATEWAY_API_KEY_ENV_VARS, &["AI_GATEWAY_API_KEY"]);
    }
}
