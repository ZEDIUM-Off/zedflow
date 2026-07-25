//! Z.AI provider factory ported from Pi's `packages/ai/src/providers/zai.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Z.AI provider id used by Pi.
pub const ZAI_PROVIDER_ID: &str = "zai";

/// Z.AI display name used by Pi.
pub const ZAI_PROVIDER_NAME: &str = "Z.AI";

/// Z.AI OpenAI-compatible API base URL used by Pi.
pub const ZAI_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";

/// Z.AI stream API id used by Pi models.
pub const ZAI_API: &str = "openai-completions";

/// Z.AI API-key auth prompt label used by Pi.
pub const ZAI_API_KEY_AUTH_NAME: &str = "Z.AI API key";

/// Environment variables checked for Z.AI API-key auth, in Pi precedence order.
pub const ZAI_API_KEY_ENV_VARS: &[&str] = &["ZAI_API_KEY"];

/// Creates the zai provider from the static Rust model catalog.
pub fn zai_provider() -> Result<Provider> {
    let provider = static_provider(
        ZAI_PROVIDER_ID,
        ZAI_PROVIDER_NAME,
        crate::providers::zai_models::zai_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = zai_provider().expect("provider");
        assert_eq!(provider.id, ZAI_PROVIDER_ID);
        assert_eq!(provider.name, ZAI_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
