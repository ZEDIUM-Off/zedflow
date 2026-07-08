//! MiniMax CN provider factory ported from Pi's `packages/ai/src/providers/minimax-cn.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// MiniMax CN provider id used by Pi.
pub const MINIMAX_CN_PROVIDER_ID: &str = "minimax-cn";

/// MiniMax CN display name used by Pi.
pub const MINIMAX_CN_PROVIDER_NAME: &str = "MiniMax CN";

/// MiniMax CN Anthropic-compatible API base URL used by Pi.
pub const MINIMAX_CN_BASE_URL: &str = "https://api.minimaxi.com/anthropic";

/// MiniMax CN stream API id used by Pi models.
pub const MINIMAX_CN_API: &str = "anthropic-messages";

/// MiniMax CN API-key auth prompt label used by Pi.
pub const MINIMAX_CN_API_KEY_AUTH_NAME: &str = "MiniMax CN API key";

/// Environment variables checked for MiniMax CN API-key auth, in Pi precedence order.
pub const MINIMAX_CN_API_KEY_ENV_VARS: &[&str] = &["MINIMAX_CN_API_KEY"];

/// Creates Pi's MiniMax CN provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/minimax-cn.models.ts MINIMAX_CN_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "minimax-cn", name: "MiniMax CN", baseUrl: "https://api.minimaxi.com/anthropic", auth: { apiKey: envApiKeyAuth("MiniMax CN API key", ["MINIMAX_CN_API_KEY"]) }, models: Object.values(MINIMAX_CN_MODELS), api: anthropicMessagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// MiniMax CN model catalog are available in Rust.
#[must_use]
pub fn minimax_cn_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/providers/minimax-cn.models.ts MINIMAX_CN_MODELS",
        "return createProvider({ id: \"minimax-cn\", name: \"MiniMax CN\", baseUrl: \"https://api.minimaxi.com/anthropic\", auth: { apiKey: envApiKeyAuth(\"MiniMax CN API key\", [\"MINIMAX_CN_API_KEY\"]) }, models: Object.values(MINIMAX_CN_MODELS), api: anthropicMessagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_minimax_cn_provider_blocker() {
        match minimax_cn_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("MINIMAX_CN_MODELS")
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
    fn preserves_minimax_cn_provider_constants() {
        assert_eq!(MINIMAX_CN_PROVIDER_ID, "minimax-cn");
        assert_eq!(MINIMAX_CN_PROVIDER_NAME, "MiniMax CN");
        assert_eq!(MINIMAX_CN_BASE_URL, "https://api.minimaxi.com/anthropic");
        assert_eq!(MINIMAX_CN_API, "anthropic-messages");
        assert_eq!(MINIMAX_CN_API_KEY_AUTH_NAME, "MiniMax CN API key");
        assert_eq!(MINIMAX_CN_API_KEY_ENV_VARS, &["MINIMAX_CN_API_KEY"]);
    }
}
