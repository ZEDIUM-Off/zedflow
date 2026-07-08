//! Xiaomi Token Plan CN provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-cn.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Xiaomi Token Plan CN provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xiaomi-token-plan-cn.models.ts XIAOMI_TOKEN_PLAN_CN_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "xiaomi-token-plan-cn", name: "Xiaomi Token Plan CN", baseUrl: "https://token-plan-cn.xiaomimimo.com/v1", auth: { apiKey: envApiKeyAuth("Xiaomi Token Plan CN API key", ["XIAOMI_TOKEN_PLAN_CN_API_KEY"]) }, models: Object.values(XIAOMI_TOKEN_PLAN_CN_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Xiaomi Token Plan CN model catalog are available in Rust.
#[must_use]
pub fn xiaomi_token_plan_cn_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xiaomi-token-plan-cn.models.ts XIAOMI_TOKEN_PLAN_CN_MODELS",
        "return createProvider({ id: \"xiaomi-token-plan-cn\", name: \"Xiaomi Token Plan CN\", baseUrl: \"https://token-plan-cn.xiaomimimo.com/v1\", auth: { apiKey: envApiKeyAuth(\"Xiaomi Token Plan CN API key\", [\"XIAOMI_TOKEN_PLAN_CN_API_KEY\"]) }, models: Object.values(XIAOMI_TOKEN_PLAN_CN_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_xiaomi_token_plan_cn_provider_blocker() {
        match xiaomi_token_plan_cn_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("XIAOMI_TOKEN_PLAN_CN_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("XIAOMI_TOKEN_PLAN_CN_API_KEY")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_xiaomi_token_plan_cn_provider_constants() {
        assert_eq!(XIAOMI_TOKEN_PLAN_CN_PROVIDER_ID, "xiaomi-token-plan-cn");
        assert_eq!(XIAOMI_TOKEN_PLAN_CN_PROVIDER_NAME, "Xiaomi Token Plan CN");
        assert_eq!(
            XIAOMI_TOKEN_PLAN_CN_BASE_URL,
            "https://token-plan-cn.xiaomimimo.com/v1"
        );
        assert_eq!(XIAOMI_TOKEN_PLAN_CN_API, "openai-completions");
        assert_eq!(
            XIAOMI_TOKEN_PLAN_CN_API_KEY_AUTH_NAME,
            "Xiaomi Token Plan CN API key"
        );
        assert_eq!(
            XIAOMI_TOKEN_PLAN_CN_API_KEY_ENV_VARS,
            &["XIAOMI_TOKEN_PLAN_CN_API_KEY"]
        );
    }
}
