//! Google provider factory ported from Pi's `packages/ai/src/providers/google.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Google provider id used by Pi.
pub const GOOGLE_PROVIDER_ID: &str = "google";

/// Google display name used by Pi.
pub const GOOGLE_PROVIDER_NAME: &str = "Google";

/// Google Generative Language API base URL used by Pi.
pub const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Generative AI stream API id used by Pi models.
pub const GOOGLE_API: &str = "google-generative-ai";

/// Gemini API-key auth prompt label used by Pi.
pub const GOOGLE_API_KEY_AUTH_NAME: &str = "Gemini API key";

/// Environment variables checked for Gemini API-key auth, in Pi precedence order.
pub const GOOGLE_API_KEY_ENV_VARS: &[&str] = &["GEMINI_API_KEY"];

/// Creates Pi's Google provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/google-generative-ai.lazy.ts googleGenerativeAIApi, references/pi/packages/ai/src/providers/google.models.ts GOOGLE_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "google", name: "Google", baseUrl: "https://generativelanguage.googleapis.com/v1beta", auth: { apiKey: envApiKeyAuth("Gemini API key", ["GEMINI_API_KEY"]) }, models: Object.values(GOOGLE_MODELS), api: googleGenerativeAIApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Google model catalog are available in Rust.
#[must_use]
pub fn google_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/google-generative-ai.lazy.ts googleGenerativeAIApi, references/pi/packages/ai/src/providers/google.models.ts GOOGLE_MODELS",
        "return createProvider({ id: \"google\", name: \"Google\", baseUrl: \"https://generativelanguage.googleapis.com/v1beta\", auth: { apiKey: envApiKeyAuth(\"Gemini API key\", [\"GEMINI_API_KEY\"]) }, models: Object.values(GOOGLE_MODELS), api: googleGenerativeAIApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_google_provider_blocker() {
        match google_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("GOOGLE_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("googleGenerativeAIApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_google_provider_constants() {
        assert_eq!(GOOGLE_PROVIDER_ID, "google");
        assert_eq!(GOOGLE_PROVIDER_NAME, "Google");
        assert_eq!(
            GOOGLE_BASE_URL,
            "https://generativelanguage.googleapis.com/v1beta"
        );
        assert_eq!(GOOGLE_API, "google-generative-ai");
        assert_eq!(GOOGLE_API_KEY_AUTH_NAME, "Gemini API key");
        assert_eq!(GOOGLE_API_KEY_ENV_VARS, &["GEMINI_API_KEY"]);
    }
}
