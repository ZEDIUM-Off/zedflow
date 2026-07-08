//! Mistral provider factory ported from Pi's `packages/ai/src/providers/mistral.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Mistral provider id used by Pi.
pub const MISTRAL_PROVIDER_ID: &str = "mistral";

/// Mistral display name used by Pi.
pub const MISTRAL_PROVIDER_NAME: &str = "Mistral";

/// Mistral API base URL used by Pi.
pub const MISTRAL_BASE_URL: &str = "https://api.mistral.ai";

/// Mistral API-key auth prompt label used by Pi.
pub const MISTRAL_API_KEY_AUTH_NAME: &str = "Mistral API key";

/// Environment variables checked for Mistral API-key auth, in Pi precedence order.
pub const MISTRAL_API_KEY_ENV_VARS: &[&str] = &["MISTRAL_API_KEY"];

/// Mistral stream API id used by Pi models.
pub const MISTRAL_API: &str = "mistral-conversations";

/// Creates Pi's Mistral provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/mistral-conversations.lazy.ts mistralConversationsApi, references/pi/packages/ai/src/providers/mistral.models.ts MISTRAL_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "mistral", name: "Mistral", baseUrl: "https://api.mistral.ai", auth: { apiKey: envApiKeyAuth("Mistral API key", ["MISTRAL_API_KEY"]) }, models: Object.values(MISTRAL_MODELS), api: mistralConversationsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract
/// and Mistral model catalog are available in Rust.
#[must_use]
pub fn mistral_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/mistral-conversations.lazy.ts mistralConversationsApi, references/pi/packages/ai/src/providers/mistral.models.ts MISTRAL_MODELS",
        "return createProvider({ id: \"mistral\", name: \"Mistral\", baseUrl: \"https://api.mistral.ai\", auth: { apiKey: envApiKeyAuth(\"Mistral API key\", [\"MISTRAL_API_KEY\"]) }, models: Object.values(MISTRAL_MODELS), api: mistralConversationsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_mistral_provider_blocker() {
        match mistral_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("MISTRAL_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("mistralConversationsApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_mistral_provider_constants() {
        assert_eq!(MISTRAL_PROVIDER_ID, "mistral");
        assert_eq!(MISTRAL_PROVIDER_NAME, "Mistral");
        assert_eq!(MISTRAL_BASE_URL, "https://api.mistral.ai");
        assert_eq!(MISTRAL_API_KEY_AUTH_NAME, "Mistral API key");
        assert_eq!(MISTRAL_API_KEY_ENV_VARS, &["MISTRAL_API_KEY"]);
        assert_eq!(MISTRAL_API, "mistral-conversations");
    }
}
