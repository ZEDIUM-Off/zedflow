//! Kimi Coding provider factory ported from Pi's `packages/ai/src/providers/kimi-coding.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

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

/// Creates the kimi-coding provider from the static Rust model catalog.
pub fn kimi_coding_provider() -> Result<Provider> {
    let provider = static_provider(
        KIMI_CODING_PROVIDER_ID,
        KIMI_CODING_PROVIDER_NAME,
        crate::providers::kimi_coding_models::kimi_coding_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = kimi_coding_provider().expect("provider");
        assert_eq!(provider.id, KIMI_CODING_PROVIDER_ID);
        assert_eq!(provider.name, KIMI_CODING_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
