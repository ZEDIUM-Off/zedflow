//! MiniMax provider factory ported from Pi's `packages/ai/src/providers/minimax.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

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

/// Creates the minimax provider from the static Rust model catalog.
pub fn minimax_provider() -> Result<Provider> {
    let provider = static_provider(
        MINIMAX_PROVIDER_ID,
        MINIMAX_PROVIDER_NAME,
        models_from_catalog(crate::providers::minimax_models::MINIMAX_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = minimax_provider().expect("provider");
        assert_eq!(provider.id, MINIMAX_PROVIDER_ID);
        assert_eq!(provider.name, MINIMAX_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
