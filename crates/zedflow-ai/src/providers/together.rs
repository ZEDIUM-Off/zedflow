//! Together provider factory ported from Pi's `packages/ai/src/providers/together.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Together provider id used by Pi.
pub const TOGETHER_PROVIDER_ID: &str = "together";

/// Together display name used by Pi.
pub const TOGETHER_PROVIDER_NAME: &str = "Together";

/// Together OpenAI-compatible API base URL used by Pi.
pub const TOGETHER_BASE_URL: &str = "https://api.together.ai/v1";

/// Together stream API id used by Pi models.
pub const TOGETHER_API: &str = "openai-completions";

/// Together API-key auth prompt label used by Pi.
pub const TOGETHER_API_KEY_AUTH_NAME: &str = "Together API key";

/// Environment variables checked for Together API-key auth, in Pi precedence order.
pub const TOGETHER_API_KEY_ENV_VARS: &[&str] = &["TOGETHER_API_KEY"];

/// Creates Pi's Together provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/together.models.ts TOGETHER_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "together", name: "Together", baseUrl: "https://api.together.ai/v1", auth: { apiKey: envApiKeyAuth("Together API key", ["TOGETHER_API_KEY"]) }, models: Object.values(TOGETHER_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Together model catalog are available in Rust.
#[must_use]
pub fn together_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/together.models.ts TOGETHER_MODELS",
        "return createProvider({ id: \"together\", name: \"Together\", baseUrl: \"https://api.together.ai/v1\", auth: { apiKey: envApiKeyAuth(\"Together API key\", [\"TOGETHER_API_KEY\"]) }, models: Object.values(TOGETHER_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_together_provider_blocker() {
        match together_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("TOGETHER_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(placeholder.required_behavior().contains("TOGETHER_API_KEY"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_together_provider_constants() {
        assert_eq!(TOGETHER_PROVIDER_ID, "together");
        assert_eq!(TOGETHER_PROVIDER_NAME, "Together");
        assert_eq!(TOGETHER_BASE_URL, "https://api.together.ai/v1");
        assert_eq!(TOGETHER_API, "openai-completions");
        assert_eq!(TOGETHER_API_KEY_AUTH_NAME, "Together API key");
        assert_eq!(TOGETHER_API_KEY_ENV_VARS, &["TOGETHER_API_KEY"]);
    }
}
