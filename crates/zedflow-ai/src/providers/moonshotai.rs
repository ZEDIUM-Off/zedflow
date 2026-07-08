//! Moonshot AI provider factory ported from Pi's `packages/ai/src/providers/moonshotai.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Moonshot AI provider id used by Pi.
pub const MOONSHOTAI_PROVIDER_ID: &str = "moonshotai";

/// Moonshot AI display name used by Pi.
pub const MOONSHOTAI_PROVIDER_NAME: &str = "Moonshot AI";

/// Moonshot AI OpenAI-compatible API base URL used by Pi.
pub const MOONSHOTAI_BASE_URL: &str = "https://api.moonshot.ai/v1";

/// Moonshot AI stream API id used by Pi models.
pub const MOONSHOTAI_API: &str = "openai-completions";

/// Moonshot AI API-key auth prompt label used by Pi.
pub const MOONSHOTAI_API_KEY_AUTH_NAME: &str = "Moonshot AI API key";

/// Environment variables checked for Moonshot AI API-key auth, in Pi precedence order.
pub const MOONSHOTAI_API_KEY_ENV_VARS: &[&str] = &["MOONSHOT_API_KEY"];

/// Creates Pi's Moonshot AI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/moonshotai.models.ts MOONSHOTAI_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "moonshotai", name: "Moonshot AI", baseUrl: "https://api.moonshot.ai/v1", auth: { apiKey: envApiKeyAuth("Moonshot AI API key", ["MOONSHOT_API_KEY"]) }, models: Object.values(MOONSHOTAI_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Moonshot AI model catalog are available in Rust.
#[must_use]
pub fn moonshotai_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/moonshotai.models.ts MOONSHOTAI_MODELS",
        "return createProvider({ id: \"moonshotai\", name: \"Moonshot AI\", baseUrl: \"https://api.moonshot.ai/v1\", auth: { apiKey: envApiKeyAuth(\"Moonshot AI API key\", [\"MOONSHOT_API_KEY\"]) }, models: Object.values(MOONSHOTAI_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_moonshotai_provider_blocker() {
        match moonshotai_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("MOONSHOTAI_MODELS")
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
    fn preserves_moonshotai_provider_constants() {
        assert_eq!(MOONSHOTAI_PROVIDER_ID, "moonshotai");
        assert_eq!(MOONSHOTAI_PROVIDER_NAME, "Moonshot AI");
        assert_eq!(MOONSHOTAI_BASE_URL, "https://api.moonshot.ai/v1");
        assert_eq!(MOONSHOTAI_API, "openai-completions");
        assert_eq!(MOONSHOTAI_API_KEY_AUTH_NAME, "Moonshot AI API key");
        assert_eq!(MOONSHOTAI_API_KEY_ENV_VARS, &["MOONSHOT_API_KEY"]);
    }
}
