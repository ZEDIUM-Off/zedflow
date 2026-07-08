//! Hugging Face provider factory ported from Pi's `packages/ai/src/providers/huggingface.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Hugging Face provider id used by Pi.
pub const HUGGINGFACE_PROVIDER_ID: &str = "huggingface";

/// Hugging Face display name used by Pi.
pub const HUGGINGFACE_PROVIDER_NAME: &str = "Hugging Face";

/// Hugging Face OpenAI-compatible router base URL used by Pi.
pub const HUGGINGFACE_BASE_URL: &str = "https://router.huggingface.co/v1";

/// Hugging Face API id used by Pi models.
pub const HUGGINGFACE_API: &str = "openai-completions";

/// Hugging Face token auth prompt label used by Pi.
pub const HUGGINGFACE_API_KEY_AUTH_NAME: &str = "Hugging Face token";

/// Environment variables checked for Hugging Face token auth, in Pi precedence order.
pub const HUGGINGFACE_API_KEY_ENV_VARS: &[&str] = &["HF_TOKEN"];

/// Creates Pi's Hugging Face provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/huggingface.models.ts HUGGINGFACE_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "huggingface", name: "Hugging Face", baseUrl: "https://router.huggingface.co/v1", auth: { apiKey: envApiKeyAuth("Hugging Face token", ["HF_TOKEN"]) }, models: Object.values(HUGGINGFACE_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Hugging Face model catalog are available in Rust.
#[must_use]
pub fn huggingface_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/huggingface.models.ts HUGGINGFACE_MODELS",
        "return createProvider({ id: \"huggingface\", name: \"Hugging Face\", baseUrl: \"https://router.huggingface.co/v1\", auth: { apiKey: envApiKeyAuth(\"Hugging Face token\", [\"HF_TOKEN\"]) }, models: Object.values(HUGGINGFACE_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_huggingface_provider_blocker() {
        match huggingface_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("HUGGINGFACE_MODELS")
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
    fn preserves_huggingface_provider_constants() {
        assert_eq!(HUGGINGFACE_PROVIDER_ID, "huggingface");
        assert_eq!(HUGGINGFACE_PROVIDER_NAME, "Hugging Face");
        assert_eq!(HUGGINGFACE_BASE_URL, "https://router.huggingface.co/v1");
        assert_eq!(HUGGINGFACE_API, "openai-completions");
        assert_eq!(HUGGINGFACE_API_KEY_AUTH_NAME, "Hugging Face token");
        assert_eq!(HUGGINGFACE_API_KEY_ENV_VARS, &["HF_TOKEN"]);
    }
}
