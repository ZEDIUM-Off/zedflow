//! Cerebras provider factory ported from Pi's `packages/ai/src/providers/cerebras.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Cerebras provider id used by Pi.
pub const CEREBRAS_PROVIDER_ID: &str = "cerebras";

/// Cerebras display name used by Pi.
pub const CEREBRAS_PROVIDER_NAME: &str = "Cerebras";

/// Cerebras OpenAI-compatible API base URL used by Pi.
pub const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Cerebras stream API id used by Pi models.
pub const CEREBRAS_API: &str = "openai-completions";

/// Cerebras API-key auth prompt label used by Pi.
pub const CEREBRAS_API_KEY_AUTH_NAME: &str = "Cerebras API key";

/// Environment variables checked for Cerebras API-key auth, in Pi precedence order.
pub const CEREBRAS_API_KEY_ENV_VARS: &[&str] = &["CEREBRAS_API_KEY"];

/// Creates Pi's Cerebras provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/cerebras.models.ts CEREBRAS_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "cerebras", name: "Cerebras", baseUrl: "https://api.cerebras.ai/v1", auth: { apiKey: envApiKeyAuth("Cerebras API key", ["CEREBRAS_API_KEY"]) }, models: Object.values(CEREBRAS_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Cerebras model catalog are available in Rust.
#[must_use]
pub fn cerebras_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/cerebras.models.ts CEREBRAS_MODELS",
        "return createProvider({ id: \"cerebras\", name: \"Cerebras\", baseUrl: \"https://api.cerebras.ai/v1\", auth: { apiKey: envApiKeyAuth(\"Cerebras API key\", [\"CEREBRAS_API_KEY\"]) }, models: Object.values(CEREBRAS_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_cerebras_provider_blocker() {
        match cerebras_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("CEREBRAS_MODELS")
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
    fn preserves_cerebras_provider_constants() {
        assert_eq!(CEREBRAS_PROVIDER_ID, "cerebras");
        assert_eq!(CEREBRAS_PROVIDER_NAME, "Cerebras");
        assert_eq!(CEREBRAS_BASE_URL, "https://api.cerebras.ai/v1");
        assert_eq!(CEREBRAS_API, "openai-completions");
        assert_eq!(CEREBRAS_API_KEY_AUTH_NAME, "Cerebras API key");
        assert_eq!(CEREBRAS_API_KEY_ENV_VARS, &["CEREBRAS_API_KEY"]);
    }
}
