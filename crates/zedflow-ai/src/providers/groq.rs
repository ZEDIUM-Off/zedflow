//! Groq provider factory ported from Pi's `packages/ai/src/providers/groq.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Groq provider id used by Pi.
pub const GROQ_PROVIDER_ID: &str = "groq";

/// Groq display name used by Pi.
pub const GROQ_PROVIDER_NAME: &str = "Groq";

/// Groq OpenAI-compatible API base URL used by Pi.
pub const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1";

/// Groq stream API id used by Pi models.
pub const GROQ_API: &str = "openai-completions";

/// Groq API-key auth prompt label used by Pi.
pub const GROQ_API_KEY_AUTH_NAME: &str = "Groq API key";

/// Environment variables checked for Groq API-key auth, in Pi precedence order.
pub const GROQ_API_KEY_ENV_VARS: &[&str] = &["GROQ_API_KEY"];

/// Creates the groq provider from the static Rust model catalog.
pub fn groq_provider() -> Result<Provider> {
    let provider = static_provider(
        GROQ_PROVIDER_ID,
        GROQ_PROVIDER_NAME,
        models_from_catalog(crate::providers::groq_models::GROQ_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = groq_provider().expect("provider");
        assert_eq!(provider.id, GROQ_PROVIDER_ID);
        assert_eq!(provider.name, GROQ_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
