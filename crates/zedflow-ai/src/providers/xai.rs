//! xAI provider factory ported from Pi's `packages/ai/src/providers/xai.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's xAI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xai.models.ts XAI_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "xai", name: "xAI", baseUrl: "https://api.x.ai/v1", auth: { apiKey: envApiKeyAuth("xAI API key", ["XAI_API_KEY"]) }, models: Object.values(XAI_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// xAI model catalog are available in Rust.
#[must_use]
pub fn xai_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/xai.models.ts XAI_MODELS",
        "return createProvider({ id: \"xai\", name: \"xAI\", baseUrl: \"https://api.x.ai/v1\", auth: { apiKey: envApiKeyAuth(\"xAI API key\", [\"XAI_API_KEY\"]) }, models: Object.values(XAI_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_xai_provider_blocker() {
        match xai_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("XAI_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(placeholder.required_behavior().contains("XAI_API_KEY"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_xai_provider_constants() {
        assert_eq!(XAI_PROVIDER_ID, "xai");
        assert_eq!(XAI_PROVIDER_NAME, "xAI");
        assert_eq!(XAI_BASE_URL, "https://api.x.ai/v1");
        assert_eq!(XAI_API, "openai-completions");
        assert_eq!(XAI_API_KEY_AUTH_NAME, "xAI API key");
        assert_eq!(XAI_API_KEY_ENV_VARS, &["XAI_API_KEY"]);
    }
}
