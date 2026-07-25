//! Moonshot AI CN provider factory ported from Pi's `packages/ai/src/providers/moonshotai-cn.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Moonshot AI CN provider id used by Pi.
pub const MOONSHOTAI_CN_PROVIDER_ID: &str = "moonshotai-cn";

/// Moonshot AI CN display name used by Pi.
pub const MOONSHOTAI_CN_PROVIDER_NAME: &str = "Moonshot AI CN";

/// Moonshot AI CN OpenAI-compatible API base URL used by Pi.
pub const MOONSHOTAI_CN_BASE_URL: &str = "https://api.moonshot.cn/v1";

/// Moonshot AI CN stream API id used by Pi models.
pub const MOONSHOTAI_CN_API: &str = "openai-completions";

/// Moonshot AI CN API-key auth prompt label used by Pi.
pub const MOONSHOTAI_CN_API_KEY_AUTH_NAME: &str = "Moonshot AI API key";

/// Environment variables checked for Moonshot AI CN API-key auth, in Pi precedence order.
pub const MOONSHOTAI_CN_API_KEY_ENV_VARS: &[&str] = &["MOONSHOT_API_KEY"];

/// Creates the moonshotai-cn provider from the static Rust model catalog.
pub fn moonshotai_cn_provider() -> Result<Provider> {
    let provider = static_provider(
        MOONSHOTAI_CN_PROVIDER_ID,
        MOONSHOTAI_CN_PROVIDER_NAME,
        crate::providers::moonshotai_cn_models::moonshotai_cn_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = moonshotai_cn_provider().expect("provider");
        assert_eq!(provider.id, MOONSHOTAI_CN_PROVIDER_ID);
        assert_eq!(provider.name, MOONSHOTAI_CN_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
