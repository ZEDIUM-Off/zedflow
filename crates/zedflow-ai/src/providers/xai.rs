//! xAI provider factory ported from Pi's `packages/ai/src/providers/xai.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::{models_from_catalog, static_provider};

/// xAI provider id used by Pi.
pub const XAI_PROVIDER_ID: &str = "xai";

/// xAI display name used by Pi.
pub const XAI_PROVIDER_NAME: &str = "xAI";

/// xAI OpenAI-compatible API base URL used by Pi.
pub const XAI_BASE_URL: &str = "https://api.x.ai/v1";

/// xAI stream API id used by Pi models.
pub const XAI_API: &str = "openai-completions";

/// xAI API-key auth prompt label used by Pi.
pub const XAI_API_KEY_AUTH_NAME: &str = "xAI API key";

/// Environment variables checked for xAI API-key auth, in Pi precedence order.
pub const XAI_API_KEY_ENV_VARS: &[&str] = &["XAI_API_KEY"];

/// Creates the xai provider from the static Rust model catalog.
pub fn xai_provider() -> Result<Provider> {
    let provider = static_provider(
        XAI_PROVIDER_ID,
        XAI_PROVIDER_NAME,
        models_from_catalog(crate::providers::xai_models::XAI_MODELS),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = xai_provider().expect("provider");
        assert_eq!(provider.id, XAI_PROVIDER_ID);
        assert_eq!(provider.name, XAI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
