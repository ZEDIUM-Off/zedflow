//! Xiaomi Token Plan SGP provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-sgp.ts`.

use zedflow_core::error::Result;

use crate::models::Provider;
use crate::providers::static_catalog::static_provider;

/// Xiaomi Token Plan SGP provider id used by Pi.
pub const XIAOMI_TOKEN_PLAN_SGP_PROVIDER_ID: &str = "xiaomi-token-plan-sgp";

/// Xiaomi Token Plan SGP display name used by Pi.
pub const XIAOMI_TOKEN_PLAN_SGP_PROVIDER_NAME: &str = "Xiaomi Token Plan SGP";

/// Xiaomi Token Plan SGP OpenAI-compatible API base URL used by Pi.
pub const XIAOMI_TOKEN_PLAN_SGP_BASE_URL: &str = "https://token-plan-sgp.xiaomimimo.com/v1";

/// Xiaomi Token Plan SGP stream API id used by Pi models.
pub const XIAOMI_TOKEN_PLAN_SGP_API: &str = "openai-completions";

/// Xiaomi Token Plan SGP API-key auth prompt label used by Pi.
pub const XIAOMI_TOKEN_PLAN_SGP_API_KEY_AUTH_NAME: &str = "Xiaomi Token Plan SGP API key";

/// Environment variables checked for Xiaomi Token Plan SGP API-key auth, in Pi precedence order.
pub const XIAOMI_TOKEN_PLAN_SGP_API_KEY_ENV_VARS: &[&str] = &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"];

/// Creates the xiaomi-token-plan-sgp provider from the static Rust model catalog.
pub fn xiaomi_token_plan_sgp_provider() -> Result<Provider> {
    let provider = static_provider(
        XIAOMI_TOKEN_PLAN_SGP_PROVIDER_ID,
        XIAOMI_TOKEN_PLAN_SGP_PROVIDER_NAME,
        crate::providers::xiaomi_token_plan_sgp_models::xiaomi_token_plan_sgp_models(),
    );
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = xiaomi_token_plan_sgp_provider().expect("provider");
        assert_eq!(provider.id, XIAOMI_TOKEN_PLAN_SGP_PROVIDER_ID);
        assert_eq!(provider.name, XIAOMI_TOKEN_PLAN_SGP_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
