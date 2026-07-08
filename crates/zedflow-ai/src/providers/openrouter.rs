//! OpenRouter provider factory ported from Pi's `packages/ai/src/providers/openrouter.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// OpenRouter provider id used by Pi.
pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";

/// OpenRouter display name used by Pi.
pub const OPENROUTER_PROVIDER_NAME: &str = "OpenRouter";

/// OpenRouter API base URL used by Pi.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter stream API id used by Pi models.
pub const OPENROUTER_API: &str = "openai-completions";

/// OpenRouter API-key auth prompt label used by Pi.
pub const OPENROUTER_API_KEY_AUTH_NAME: &str = "OpenRouter API key";

/// Environment variables checked for OpenRouter API-key auth, in Pi precedence order.
pub const OPENROUTER_API_KEY_ENV_VARS: &[&str] = &["OPENROUTER_API_KEY"];

/// Creates Pi's OpenRouter provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/openrouter.models.ts OPENROUTER_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "openrouter", name: "OpenRouter", baseUrl: "https://openrouter.ai/api/v1", auth: { apiKey: envApiKeyAuth("OpenRouter API key", ["OPENROUTER_API_KEY"]) }, models: Object.values(OPENROUTER_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// OpenRouter model catalog are available in Rust.
#[must_use]
pub fn openrouter_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/openrouter.models.ts OPENROUTER_MODELS",
        "return createProvider({ id: \"openrouter\", name: \"OpenRouter\", baseUrl: \"https://openrouter.ai/api/v1\", auth: { apiKey: envApiKeyAuth(\"OpenRouter API key\", [\"OPENROUTER_API_KEY\"]) }, models: Object.values(OPENROUTER_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_openrouter_provider_blocker() {
        match openrouter_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("OPENROUTER_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("OPENROUTER_API_KEY")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_openrouter_provider_constants() {
        assert_eq!(OPENROUTER_PROVIDER_ID, "openrouter");
        assert_eq!(OPENROUTER_PROVIDER_NAME, "OpenRouter");
        assert_eq!(OPENROUTER_BASE_URL, "https://openrouter.ai/api/v1");
        assert_eq!(OPENROUTER_API, "openai-completions");
        assert_eq!(OPENROUTER_API_KEY_AUTH_NAME, "OpenRouter API key");
        assert_eq!(OPENROUTER_API_KEY_ENV_VARS, &["OPENROUTER_API_KEY"]);
    }
}
