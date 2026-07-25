//! Xiaomi Token Plan CN provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-cn.ts`.

use crate::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Xiaomi Token Plan CN provider id used by Pi.
pub const XIAOMI_TOKEN_PLAN_CN_PROVIDER_ID: &str = "xiaomi-token-plan-cn";

/// Xiaomi Token Plan CN display name used by Pi.
pub const XIAOMI_TOKEN_PLAN_CN_PROVIDER_NAME: &str = "Xiaomi Token Plan CN";

/// Xiaomi Token Plan CN OpenAI-compatible API base URL used by Pi.
pub const XIAOMI_TOKEN_PLAN_CN_BASE_URL: &str = "https://token-plan-cn.xiaomimimo.com/v1";

/// Xiaomi Token Plan CN stream API id used by Pi models.
pub const XIAOMI_TOKEN_PLAN_CN_API: &str = "openai-completions";

/// Xiaomi Token Plan CN API-key auth prompt label used by Pi.
pub const XIAOMI_TOKEN_PLAN_CN_API_KEY_AUTH_NAME: &str = "Xiaomi Token Plan CN API key";

/// Environment variables checked for Xiaomi Token Plan CN API-key auth, in Pi precedence order.
pub const XIAOMI_TOKEN_PLAN_CN_API_KEY_ENV_VARS: &[&str] = &["XIAOMI_TOKEN_PLAN_CN_API_KEY"];

/// Creates the xiaomi-token-plan-cn provider from the static Rust model catalog.
pub fn xiaomi_token_plan_cn_provider() -> Result<Provider> {
    let provider = static_provider(
        XIAOMI_TOKEN_PLAN_CN_PROVIDER_ID,
        XIAOMI_TOKEN_PLAN_CN_PROVIDER_NAME,
        crate::providers::xiaomi_token_plan_cn_models::xiaomi_token_plan_cn_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = xiaomi_token_plan_cn_provider().expect("provider");
        assert_eq!(provider.id, XIAOMI_TOKEN_PLAN_CN_PROVIDER_ID);
        assert_eq!(provider.name, XIAOMI_TOKEN_PLAN_CN_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
