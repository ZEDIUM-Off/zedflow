//! Xiaomi provider factory ported from Pi's `packages/ai/src/providers/xiaomi.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

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

/// Creates the xiaomi provider from the static Rust model catalog.
pub fn xiaomi_provider() -> Result<Provider> {
    let provider = static_provider(
        XIAOMI_PROVIDER_ID,
        XIAOMI_PROVIDER_NAME,
        models_from_catalog(crate::providers::xiaomi_models::XIAOMI_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = xiaomi_provider().expect("provider");
        assert_eq!(provider.id, XIAOMI_PROVIDER_ID);
        assert_eq!(provider.name, XIAOMI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
