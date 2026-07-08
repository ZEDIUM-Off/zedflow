//! Z.AI provider factory ported from Pi's `packages/ai/src/providers/zai.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

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

/// Creates Pi's Z.AI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/zai.models.ts ZAI_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "zai", name: "Z.AI", baseUrl: "https://api.z.ai/api/coding/paas/v4", auth: { apiKey: envApiKeyAuth("Z.AI API key", ["ZAI_API_KEY"]) }, models: Object.values(ZAI_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Z.AI model catalog are available in Rust.
#[must_use]
pub fn zai_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/zai.models.ts ZAI_MODELS",
        "return createProvider({ id: \"zai\", name: \"Z.AI\", baseUrl: \"https://api.z.ai/api/coding/paas/v4\", auth: { apiKey: envApiKeyAuth(\"Z.AI API key\", [\"ZAI_API_KEY\"]) }, models: Object.values(ZAI_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_zai_provider_blocker() {
        match zai_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("ZAI_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(placeholder.required_behavior().contains("ZAI_API_KEY"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_zai_provider_constants() {
        assert_eq!(ZAI_PROVIDER_ID, "zai");
        assert_eq!(ZAI_PROVIDER_NAME, "Z.AI");
        assert_eq!(ZAI_BASE_URL, "https://api.z.ai/api/coding/paas/v4");
        assert_eq!(ZAI_API, "openai-completions");
        assert_eq!(ZAI_API_KEY_AUTH_NAME, "Z.AI API key");
        assert_eq!(ZAI_API_KEY_ENV_VARS, &["ZAI_API_KEY"]);
    }
}
