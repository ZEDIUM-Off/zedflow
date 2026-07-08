//! DeepSeek provider factory ported from Pi's `packages/ai/src/providers/deepseek.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's DeepSeek provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/deepseek.models.ts DEEPSEEK_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com", auth: { apiKey: envApiKeyAuth("DeepSeek API key", ["DEEPSEEK_API_KEY"]) }, models: Object.values(DEEPSEEK_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// DeepSeek model catalog are available in Rust.
#[must_use]
pub fn deepseek_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/deepseek.models.ts DEEPSEEK_MODELS",
        "return createProvider({ id: \"deepseek\", name: \"DeepSeek\", baseUrl: \"https://api.deepseek.com\", auth: { apiKey: envApiKeyAuth(\"DeepSeek API key\", [\"DEEPSEEK_API_KEY\"]) }, models: Object.values(DEEPSEEK_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_deepseek_provider_blocker() {
        match deepseek_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("DEEPSEEK_MODELS")
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
    fn preserves_deepseek_provider_constants() {
        assert_eq!(DEEPSEEK_PROVIDER_ID, "deepseek");
        assert_eq!(DEEPSEEK_PROVIDER_NAME, "DeepSeek");
        assert_eq!(DEEPSEEK_BASE_URL, "https://api.deepseek.com");
        assert_eq!(DEEPSEEK_API, "openai-completions");
        assert_eq!(DEEPSEEK_API_KEY_AUTH_NAME, "DeepSeek API key");
        assert_eq!(DEEPSEEK_API_KEY_ENV_VARS, &["DEEPSEEK_API_KEY"]);
    }
}
