//! Cerebras provider factory ported from Pi's `packages/ai/src/providers/cerebras.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Cerebras provider id used by Pi.
pub const CEREBRAS_PROVIDER_ID: &str = "cerebras";

/// Cerebras display name used by Pi.
pub const CEREBRAS_PROVIDER_NAME: &str = "Cerebras";

/// Cerebras OpenAI-compatible API base URL used by Pi.
pub const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Cerebras stream API id used by Pi models.
pub const CEREBRAS_API: &str = "openai-completions";

/// Cerebras API-key auth prompt label used by Pi.
pub const CEREBRAS_API_KEY_AUTH_NAME: &str = "Cerebras API key";

/// Environment variables checked for Cerebras API-key auth, in Pi precedence order.
pub const CEREBRAS_API_KEY_ENV_VARS: &[&str] = &["CEREBRAS_API_KEY"];

/// Creates the cerebras provider from the static Rust model catalog.
pub fn cerebras_provider() -> Result<Provider> {
    let provider = static_provider(
        CEREBRAS_PROVIDER_ID,
        CEREBRAS_PROVIDER_NAME,
        models_from_catalog(crate::providers::cerebras_models::CEREBRAS_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = cerebras_provider().expect("provider");
        assert_eq!(provider.id, CEREBRAS_PROVIDER_ID);
        assert_eq!(provider.name, CEREBRAS_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
