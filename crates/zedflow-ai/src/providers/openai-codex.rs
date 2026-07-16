//! OpenAI Codex provider factory ported from Pi's `packages/ai/src/providers/openai-codex.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// OpenAI Codex provider id used by Pi.
pub const OPENAI_CODEX_PROVIDER_ID: &str = "openai-codex";

/// OpenAI Codex display name used by Pi.
pub const OPENAI_CODEX_PROVIDER_NAME: &str = "OpenAI Codex";

/// OpenAI Codex backend API base URL used by Pi.
pub const OPENAI_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

/// OpenAI Codex OAuth display name used by Pi.
pub const OPENAI_CODEX_OAUTH_NAME: &str = "OpenAI (ChatGPT Plus/Pro)";

/// OpenAI Codex stream API id used by Pi models.
pub const OPENAI_CODEX_API: &str = "openai-codex-responses";

/// Creates the openai-codex provider from the static Rust model catalog.
pub fn openai_codex_provider() -> Result<Provider> {
    let provider = static_provider(
        OPENAI_CODEX_PROVIDER_ID,
        OPENAI_CODEX_PROVIDER_NAME,
        models_from_catalog(crate::providers::openai_codex_models::OPENAI_CODEX_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = openai_codex_provider().expect("provider");
        assert_eq!(provider.id, OPENAI_CODEX_PROVIDER_ID);
        assert_eq!(provider.name, OPENAI_CODEX_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
