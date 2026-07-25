//! MiniMax CN provider factory ported from Pi's `packages/ai/src/providers/minimax-cn.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

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

/// Creates the minimax-cn provider from the static Rust model catalog.
pub fn minimax_cn_provider() -> Result<Provider> {
    let provider = static_provider(
        MINIMAX_CN_PROVIDER_ID,
        MINIMAX_CN_PROVIDER_NAME,
        crate::providers::minimax_cn_models::minimax_cn_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = minimax_cn_provider().expect("provider");
        assert_eq!(provider.id, MINIMAX_CN_PROVIDER_ID);
        assert_eq!(provider.name, MINIMAX_CN_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
