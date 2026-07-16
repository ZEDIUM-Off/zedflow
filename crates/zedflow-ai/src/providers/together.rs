//! Together provider factory ported from Pi's `packages/ai/src/providers/together.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Together provider id used by Pi.
pub const TOGETHER_PROVIDER_ID: &str = "together";

/// Together display name used by Pi.
pub const TOGETHER_PROVIDER_NAME: &str = "Together";

/// Together OpenAI-compatible API base URL used by Pi.
pub const TOGETHER_BASE_URL: &str = "https://api.together.ai/v1";

/// Together stream API id used by Pi models.
pub const TOGETHER_API: &str = "openai-completions";

/// Together API-key auth prompt label used by Pi.
pub const TOGETHER_API_KEY_AUTH_NAME: &str = "Together API key";

/// Environment variables checked for Together API-key auth, in Pi precedence order.
pub const TOGETHER_API_KEY_ENV_VARS: &[&str] = &["TOGETHER_API_KEY"];

/// Creates the together provider from the static Rust model catalog.
pub fn together_provider() -> Result<Provider> {
    let provider = static_provider(
        TOGETHER_PROVIDER_ID,
        TOGETHER_PROVIDER_NAME,
        models_from_catalog(crate::providers::together_models::TOGETHER_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = together_provider().expect("provider");
        assert_eq!(provider.id, TOGETHER_PROVIDER_ID);
        assert_eq!(provider.name, TOGETHER_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
