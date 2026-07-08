//! Groq provider factory ported from Pi's `packages/ai/src/providers/groq.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Groq provider id used by Pi.
pub const GROQ_PROVIDER_ID: &str = "groq";

/// Groq display name used by Pi.
pub const GROQ_PROVIDER_NAME: &str = "Groq";

/// Groq OpenAI-compatible API base URL used by Pi.
pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";

/// Groq stream API id used by Pi models.
pub const GROQ_API: &str = "openai-completions";

/// Groq API-key auth prompt label used by Pi.
pub const GROQ_API_KEY_AUTH_NAME: &str = "Groq API key";

/// Environment variables checked for Groq API-key auth, in Pi precedence order.
pub const GROQ_API_KEY_ENV_VARS: &[&str] = &["GROQ_API_KEY"];

/// Creates Pi's Groq provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/groq.models.ts GROQ_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "groq", name: "Groq", baseUrl: "https://api.groq.com/openai/v1", auth: { apiKey: envApiKeyAuth("Groq API key", ["GROQ_API_KEY"]) }, models: Object.values(GROQ_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Groq model catalog are available in Rust.
#[must_use]
pub fn groq_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/groq.models.ts GROQ_MODELS",
        "return createProvider({ id: \"groq\", name: \"Groq\", baseUrl: \"https://api.groq.com/openai/v1\", auth: { apiKey: envApiKeyAuth(\"Groq API key\", [\"GROQ_API_KEY\"]) }, models: Object.values(GROQ_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_groq_provider_blocker() {
        match groq_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("GROQ_MODELS"));
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
    fn preserves_groq_provider_constants() {
        assert_eq!(GROQ_PROVIDER_ID, "groq");
        assert_eq!(GROQ_PROVIDER_NAME, "Groq");
        assert_eq!(GROQ_BASE_URL, "https://api.groq.com/openai/v1");
        assert_eq!(GROQ_API, "openai-completions");
        assert_eq!(GROQ_API_KEY_AUTH_NAME, "Groq API key");
        assert_eq!(GROQ_API_KEY_ENV_VARS, &["GROQ_API_KEY"]);
    }
}
