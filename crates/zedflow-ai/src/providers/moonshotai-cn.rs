//! Moonshot AI CN provider factory ported from Pi's `packages/ai/src/providers/moonshotai-cn.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Moonshot AI CN provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/moonshotai-cn.models.ts MOONSHOTAI_CN_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "moonshotai-cn", name: "Moonshot AI CN", baseUrl: "https://api.moonshot.cn/v1", auth: { apiKey: envApiKeyAuth("Moonshot AI API key", ["MOONSHOT_API_KEY"]) }, models: Object.values(MOONSHOTAI_CN_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract is
/// available in Rust.
#[must_use]
pub fn moonshotai_cn_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/moonshotai-cn.models.ts MOONSHOTAI_CN_MODELS",
        "return createProvider({ id: \"moonshotai-cn\", name: \"Moonshot AI CN\", baseUrl: \"https://api.moonshot.cn/v1\", auth: { apiKey: envApiKeyAuth(\"Moonshot AI API key\", [\"MOONSHOT_API_KEY\"]) }, models: Object.values(MOONSHOTAI_CN_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_moonshotai_cn_provider_blocker() {
        match moonshotai_cn_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("MOONSHOTAI_CN_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_moonshotai_cn_provider_constants() {
        assert_eq!(MOONSHOTAI_CN_PROVIDER_ID, "moonshotai-cn");
        assert_eq!(MOONSHOTAI_CN_PROVIDER_NAME, "Moonshot AI CN");
        assert_eq!(MOONSHOTAI_CN_BASE_URL, "https://api.moonshot.cn/v1");
        assert_eq!(MOONSHOTAI_CN_API, "openai-completions");
        assert_eq!(MOONSHOTAI_CN_API_KEY_AUTH_NAME, "Moonshot AI API key");
        assert_eq!(MOONSHOTAI_CN_API_KEY_ENV_VARS, &["MOONSHOT_API_KEY"]);
    }
}
