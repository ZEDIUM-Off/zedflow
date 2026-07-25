//! OpenAI provider factory ported from Pi's `packages/ai/src/providers/openai.ts`.

use crate::error::Result;

use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// OpenAI provider id used by Pi.
pub const OPENAI_PROVIDER_ID: &str = "openai";

/// OpenAI display name used by Pi.
pub const OPENAI_PROVIDER_NAME: &str = "OpenAI";

/// OpenAI API base URL used by Pi.
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI stream API id used by Pi models.
pub const OPENAI_API: &str = "openai-responses";

/// OpenAI API-key auth prompt label used by Pi.
pub const OPENAI_API_KEY_AUTH_NAME: &str = "OpenAI API key";

/// Environment variables checked for OpenAI API-key auth, in Pi precedence order.
pub const OPENAI_API_KEY_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];

/// Creates the openai provider from the static Rust model catalog.
pub fn openai_provider() -> Result<Provider> {
    let mut provider = static_provider(
        OPENAI_PROVIDER_ID,
        OPENAI_PROVIDER_NAME,
        models_from_catalog(crate::providers::openai_models::OPENAI_MODELS),
    );
    provider.api = ProviderApi::Single(crate::api::openai_responses_lazy::open_ai_responses_api());
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = openai_provider().expect("provider");
        assert_eq!(provider.id, OPENAI_PROVIDER_ID);
        assert_eq!(provider.name, OPENAI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
