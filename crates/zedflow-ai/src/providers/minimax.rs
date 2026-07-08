//! MiniMax provider factory ported from Pi's `packages/ai/src/providers/minimax.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// MiniMax provider id used by Pi.
pub const MINIMAX_PROVIDER_ID: &str = "minimax";

/// MiniMax display name used by Pi.
pub const MINIMAX_PROVIDER_NAME: &str = "MiniMax";

/// MiniMax Anthropic-compatible API base URL used by Pi.
pub const MINIMAX_BASE_URL: &str = "https://api.minimax.io/anthropic";

/// MiniMax stream API id used by Pi models.
pub const MINIMAX_API: &str = "anthropic-messages";

/// MiniMax API-key auth prompt label used by Pi.
pub const MINIMAX_API_KEY_AUTH_NAME: &str = "MiniMax API key";

/// Environment variables checked for MiniMax API-key auth, in Pi precedence order.
pub const MINIMAX_API_KEY_ENV_VARS: &[&str] = &["MINIMAX_API_KEY"];

/// Creates Pi's MiniMax provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/minimax.models.ts MINIMAX_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "minimax", name: "MiniMax", baseUrl: "https://api.minimax.io/anthropic", auth: { apiKey: envApiKeyAuth("MiniMax API key", ["MINIMAX_API_KEY"]) }, models: Object.values(MINIMAX_MODELS), api: anthropicMessagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// MiniMax model catalog are available in Rust.
#[must_use]
pub fn minimax_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/minimax.models.ts MINIMAX_MODELS",
        "return createProvider({ id: \"minimax\", name: \"MiniMax\", baseUrl: \"https://api.minimax.io/anthropic\", auth: { apiKey: envApiKeyAuth(\"MiniMax API key\", [\"MINIMAX_API_KEY\"]) }, models: Object.values(MINIMAX_MODELS), api: anthropicMessagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_minimax_provider_blocker() {
        match minimax_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("MINIMAX_MODELS"));
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
    fn preserves_minimax_provider_constants() {
        assert_eq!(MINIMAX_PROVIDER_ID, "minimax");
        assert_eq!(MINIMAX_PROVIDER_NAME, "MiniMax");
        assert_eq!(MINIMAX_BASE_URL, "https://api.minimax.io/anthropic");
        assert_eq!(MINIMAX_API, "anthropic-messages");
        assert_eq!(MINIMAX_API_KEY_AUTH_NAME, "MiniMax API key");
        assert_eq!(MINIMAX_API_KEY_ENV_VARS, &["MINIMAX_API_KEY"]);
    }
}
