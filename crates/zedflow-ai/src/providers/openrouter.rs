//! OpenRouter provider factory ported from Pi's `packages/ai/src/providers/openrouter.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// OpenRouter provider id used by Pi.
pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";

/// OpenRouter display name used by Pi.
pub const OPENROUTER_PROVIDER_NAME: &str = "OpenRouter";

/// OpenRouter API base URL used by Pi.
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter stream API id used by Pi models.
pub const OPENROUTER_API: &str = "openai-completions";

/// OpenRouter API-key auth prompt label used by Pi.
pub const OPENROUTER_API_KEY_AUTH_NAME: &str = "OpenRouter API key";

/// Environment variables checked for OpenRouter API-key auth, in Pi precedence order.
pub const OPENROUTER_API_KEY_ENV_VARS: &[&str] = &["OPENROUTER_API_KEY"];

/// Creates the openrouter provider from the static Rust model catalog.
pub fn openrouter_provider() -> Result<Provider> {
    let provider = static_provider(
        OPENROUTER_PROVIDER_ID,
        OPENROUTER_PROVIDER_NAME,
        models_from_catalog(crate::providers::openrouter_models::OPENROUTER_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = openrouter_provider().expect("provider");
        assert_eq!(provider.id, OPENROUTER_PROVIDER_ID);
        assert_eq!(provider.name, OPENROUTER_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
