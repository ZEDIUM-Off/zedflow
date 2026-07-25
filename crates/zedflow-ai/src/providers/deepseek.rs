//! DeepSeek provider factory ported from Pi's `packages/ai/src/providers/deepseek.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// DeepSeek provider id used by Pi.
pub const DEEPSEEK_PROVIDER_ID: &str = "deepseek";

/// DeepSeek display name used by Pi.
pub const DEEPSEEK_PROVIDER_NAME: &str = "DeepSeek";

/// DeepSeek OpenAI-compatible API base URL used by Pi.
pub const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";

/// DeepSeek stream API id used by Pi models.
pub const DEEPSEEK_API: &str = "openai-completions";

/// DeepSeek API-key auth prompt label used by Pi.
pub const DEEPSEEK_API_KEY_AUTH_NAME: &str = "DeepSeek API key";

/// Environment variables checked for DeepSeek API-key auth, in Pi precedence order.
pub const DEEPSEEK_API_KEY_ENV_VARS: &[&str] = &["DEEPSEEK_API_KEY"];

/// Creates the deepseek provider from the static Rust model catalog.
pub fn deepseek_provider() -> Result<Provider> {
    let provider = static_provider(
        DEEPSEEK_PROVIDER_ID,
        DEEPSEEK_PROVIDER_NAME,
        crate::providers::deepseek_models::deepseek_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = deepseek_provider().expect("provider");
        assert_eq!(provider.id, DEEPSEEK_PROVIDER_ID);
        assert_eq!(provider.name, DEEPSEEK_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
