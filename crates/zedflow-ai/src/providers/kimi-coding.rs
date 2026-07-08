//! Kimi Coding provider factory ported from Pi's `packages/ai/src/providers/kimi-coding.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Kimi Coding provider id used by Pi.
pub const KIMI_CODING_PROVIDER_ID: &str = "kimi-coding";

/// Kimi Coding display name used by Pi.
pub const KIMI_CODING_PROVIDER_NAME: &str = "Kimi For Coding";

/// Kimi Coding Anthropic-compatible API base URL used by Pi.
pub const KIMI_CODING_BASE_URL: &str = "https://api.kimi.com/coding";

/// Kimi Coding stream API id used by Pi models.
pub const KIMI_CODING_API: &str = "anthropic-messages";

/// Kimi Coding API-key auth prompt label used by Pi.
pub const KIMI_CODING_API_KEY_AUTH_NAME: &str = "Kimi API key";

/// Environment variables checked for Kimi Coding API-key auth, in Pi precedence order.
pub const KIMI_CODING_API_KEY_ENV_VARS: &[&str] = &["KIMI_API_KEY"];

/// Creates Pi's Kimi Coding provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/kimi-coding.models.ts KIMI_CODING_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "kimi-coding", name: "Kimi For Coding", baseUrl: "https://api.kimi.com/coding", auth: { apiKey: envApiKeyAuth("Kimi API key", ["KIMI_API_KEY"]) }, models: Object.values(KIMI_CODING_MODELS), api: anthropicMessagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract is available in Rust.
#[must_use]
pub fn kimi_coding_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/kimi-coding.models.ts KIMI_CODING_MODELS",
        "return createProvider({ id: \"kimi-coding\", name: \"Kimi For Coding\", baseUrl: \"https://api.kimi.com/coding\", auth: { apiKey: envApiKeyAuth(\"Kimi API key\", [\"KIMI_API_KEY\"]) }, models: Object.values(KIMI_CODING_MODELS), api: anthropicMessagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_kimi_coding_provider_blocker() {
        match kimi_coding_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("KIMI_CODING_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("anthropicMessagesApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_kimi_coding_provider_constants() {
        assert_eq!(KIMI_CODING_PROVIDER_ID, "kimi-coding");
        assert_eq!(KIMI_CODING_PROVIDER_NAME, "Kimi For Coding");
        assert_eq!(KIMI_CODING_BASE_URL, "https://api.kimi.com/coding");
        assert_eq!(KIMI_CODING_API, "anthropic-messages");
        assert_eq!(KIMI_CODING_API_KEY_AUTH_NAME, "Kimi API key");
        assert_eq!(KIMI_CODING_API_KEY_ENV_VARS, &["KIMI_API_KEY"]);
    }
}
