//! OpenCode Zen Go provider factory ported from Pi's `packages/ai/src/providers/opencode-go.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// OpenCode Zen Go provider id used by Pi.
pub const OPENCODE_GO_PROVIDER_ID: &str = "opencode-go";

/// OpenCode Zen Go display name used by Pi.
pub const OPENCODE_GO_PROVIDER_NAME: &str = "OpenCode Zen Go";

/// OpenCode Zen Go API-key auth prompt label used by Pi.
pub const OPENCODE_GO_API_KEY_AUTH_NAME: &str = "OpenCode API key";

/// Environment variables checked for OpenCode Zen Go API-key auth, in Pi precedence order.
pub const OPENCODE_GO_API_KEY_ENV_VARS: &[&str] = &["OPENCODE_API_KEY"];

/// OpenCode Zen Go stream API ids used by Pi models.
pub const OPENCODE_GO_APIS: &[&str] = &["anthropic-messages", "openai-completions"];

/// Creates the opencode-go provider from the static Rust model catalog.
pub fn opencode_go_provider() -> Result<Provider> {
    let provider = static_provider(
        OPENCODE_GO_PROVIDER_ID,
        OPENCODE_GO_PROVIDER_NAME,
        models_from_catalog(crate::providers::opencode_go_models::OPENCODE_GO_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = opencode_go_provider().expect("provider");
        assert_eq!(provider.id, OPENCODE_GO_PROVIDER_ID);
        assert_eq!(provider.name, OPENCODE_GO_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
