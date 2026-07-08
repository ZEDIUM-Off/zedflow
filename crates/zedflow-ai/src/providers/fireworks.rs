//! Fireworks provider factory ported from Pi's `packages/ai/src/providers/fireworks.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Fireworks provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/fireworks.models.ts FIREWORKS_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "fireworks", name: "Fireworks", baseUrl: "https://api.fireworks.ai/inference", auth: { apiKey: envApiKeyAuth("Fireworks API key", ["FIREWORKS_API_KEY"]) }, models: Object.values(FIREWORKS_MODELS), api: { "anthropic-messages": anthropicMessagesApi(), "openai-completions": openAICompletionsApi() } })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Fireworks model catalog are available in Rust.
#[must_use]
pub fn fireworks_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/fireworks.models.ts FIREWORKS_MODELS",
        "return createProvider({ id: \"fireworks\", name: \"Fireworks\", baseUrl: \"https://api.fireworks.ai/inference\", auth: { apiKey: envApiKeyAuth(\"Fireworks API key\", [\"FIREWORKS_API_KEY\"]) }, models: Object.values(FIREWORKS_MODELS), api: { \"anthropic-messages\": anthropicMessagesApi(), \"openai-completions\": openAICompletionsApi() } })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_fireworks_provider_blocker() {
        match fireworks_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("FIREWORKS_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("anthropicMessagesApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_fireworks_provider_constants() {
        assert_eq!(FIREWORKS_PROVIDER_ID, "fireworks");
        assert_eq!(FIREWORKS_PROVIDER_NAME, "Fireworks");
        assert_eq!(FIREWORKS_BASE_URL, "https://api.fireworks.ai/inference");
        assert_eq!(FIREWORKS_API_KEY_AUTH_NAME, "Fireworks API key");
        assert_eq!(FIREWORKS_API_KEY_ENV_VARS, &["FIREWORKS_API_KEY"]);
        assert_eq!(
            FIREWORKS_APIS,
            &["anthropic-messages", "openai-completions"]
        );
    }
}
