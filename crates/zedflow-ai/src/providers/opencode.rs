//! OpenCode Zen provider factory ported from Pi's `packages/ai/src/providers/opencode.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// OpenCode Zen provider id used by Pi.
pub const OPENCODE_PROVIDER_ID: &str = "opencode";

/// OpenCode Zen display name used by Pi.
pub const OPENCODE_PROVIDER_NAME: &str = "OpenCode Zen";

/// OpenCode Zen API-key auth prompt label used by Pi.
pub const OPENCODE_API_KEY_AUTH_NAME: &str = "OpenCode API key";

/// Environment variables checked for OpenCode Zen API-key auth, in Pi precedence order.
pub const OPENCODE_API_KEY_ENV_VARS: &[&str] = &["OPENCODE_API_KEY"];

/// OpenCode Zen stream API ids used by Pi models.
pub const OPENCODE_APIS: &[&str] = &[
    "anthropic-messages",
    "google-generative-ai",
    "openai-completions",
    "openai-responses",
];

/// Creates the opencode provider from the static Rust model catalog.
pub fn opencode_provider() -> Result<Provider> {
    let provider = static_provider(
        OPENCODE_PROVIDER_ID,
        OPENCODE_PROVIDER_NAME,
        models_from_catalog(crate::providers::opencode_models::OPENCODE_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = opencode_provider().expect("provider");
        assert_eq!(provider.id, OPENCODE_PROVIDER_ID);
        assert_eq!(provider.name, OPENCODE_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
