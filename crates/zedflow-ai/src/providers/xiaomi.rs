//! Xiaomi provider factory ported from Pi's `packages/ai/src/providers/xiaomi.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Xiaomi provider id used by Pi.
pub const XIAOMI_PROVIDER_ID: &str = "xiaomi";

/// Xiaomi display name used by Pi.
pub const XIAOMI_PROVIDER_NAME: &str = "Xiaomi";

/// Xiaomi OpenAI-compatible API base URL used by Pi.
pub const XIAOMI_BASE_URL: &str = "https://api.xiaomimimo.com/v1";

/// Xiaomi stream API id used by Pi models.
pub const XIAOMI_API: &str = "openai-completions";

/// Xiaomi API-key auth prompt label used by Pi.
pub const XIAOMI_API_KEY_AUTH_NAME: &str = "Xiaomi API key";

/// Environment variables checked for Xiaomi API-key auth, in Pi precedence order.
pub const XIAOMI_API_KEY_ENV_VARS: &[&str] = &["XIAOMI_API_KEY"];

/// Creates Pi's Xiaomi provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xiaomi.models.ts XIAOMI_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "xiaomi", name: "Xiaomi", baseUrl: "https://api.xiaomimimo.com/v1", auth: { apiKey: envApiKeyAuth("Xiaomi API key", ["XIAOMI_API_KEY"]) }, models: Object.values(XIAOMI_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Xiaomi model catalog are available in Rust.
#[must_use]
pub fn xiaomi_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xiaomi.models.ts XIAOMI_MODELS",
        "return createProvider({ id: \"xiaomi\", name: \"Xiaomi\", baseUrl: \"https://api.xiaomimimo.com/v1\", auth: { apiKey: envApiKeyAuth(\"Xiaomi API key\", [\"XIAOMI_API_KEY\"]) }, models: Object.values(XIAOMI_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_xiaomi_provider_blocker() {
        match xiaomi_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("XIAOMI_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(placeholder.required_behavior().contains("XIAOMI_API_KEY"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_xiaomi_provider_constants() {
        assert_eq!(XIAOMI_PROVIDER_ID, "xiaomi");
        assert_eq!(XIAOMI_PROVIDER_NAME, "Xiaomi");
        assert_eq!(XIAOMI_BASE_URL, "https://api.xiaomimimo.com/v1");
        assert_eq!(XIAOMI_API, "openai-completions");
        assert_eq!(XIAOMI_API_KEY_AUTH_NAME, "Xiaomi API key");
        assert_eq!(XIAOMI_API_KEY_ENV_VARS, &["XIAOMI_API_KEY"]);
    }
}
