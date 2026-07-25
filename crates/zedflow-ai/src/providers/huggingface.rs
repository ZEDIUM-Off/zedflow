//! Hugging Face provider factory ported from Pi's `packages/ai/src/providers/huggingface.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// Hugging Face provider id used by Pi.
pub const HUGGINGFACE_PROVIDER_ID: &str = "huggingface";

/// Hugging Face display name used by Pi.
pub const HUGGINGFACE_PROVIDER_NAME: &str = "Hugging Face";

/// Hugging Face OpenAI-compatible router base URL used by Pi.
pub const HUGGINGFACE_BASE_URL: &str = "https://router.huggingface.co/v1";

/// Hugging Face API id used by Pi models.
pub const HUGGINGFACE_API: &str = "openai-completions";

/// Hugging Face token auth prompt label used by Pi.
pub const HUGGINGFACE_API_KEY_AUTH_NAME: &str = "Hugging Face token";

/// Environment variables checked for Hugging Face token auth, in Pi precedence order.
pub const HUGGINGFACE_API_KEY_ENV_VARS: &[&str] = &["HF_TOKEN"];

/// Creates the huggingface provider from the static Rust model catalog.
pub fn huggingface_provider() -> Result<Provider> {
    let provider = static_provider(
        HUGGINGFACE_PROVIDER_ID,
        HUGGINGFACE_PROVIDER_NAME,
        models_from_catalog(crate::providers::huggingface_models::HUGGINGFACE_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = huggingface_provider().expect("provider");
        assert_eq!(provider.id, HUGGINGFACE_PROVIDER_ID);
        assert_eq!(provider.name, HUGGINGFACE_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
