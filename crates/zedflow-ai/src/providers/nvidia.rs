//! NVIDIA provider factory ported from Pi's `packages/ai/src/providers/nvidia.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// NVIDIA provider id used by Pi.
pub const NVIDIA_PROVIDER_ID: &str = "nvidia";

/// NVIDIA display name used by Pi.
pub const NVIDIA_PROVIDER_NAME: &str = "NVIDIA";

/// NVIDIA OpenAI-compatible API base URL used by Pi.
pub const NVIDIA_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// NVIDIA stream API id used by Pi models.
pub const NVIDIA_API: &str = "openai-completions";

/// NVIDIA API-key auth prompt label used by Pi.
pub const NVIDIA_API_KEY_AUTH_NAME: &str = "NVIDIA API key";

/// Environment variables checked for NVIDIA API-key auth, in Pi precedence order.
pub const NVIDIA_API_KEY_ENV_VARS: &[&str] = &["NVIDIA_API_KEY"];

/// Creates Pi's NVIDIA provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/nvidia.models.ts NVIDIA_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "nvidia", name: "NVIDIA", baseUrl: "https://integrate.api.nvidia.com/v1", auth: { apiKey: envApiKeyAuth("NVIDIA API key", ["NVIDIA_API_KEY"]) }, models: Object.values(NVIDIA_MODELS), api: openAICompletionsApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// NVIDIA model catalog are available in Rust.
#[must_use]
pub fn nvidia_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/nvidia.models.ts NVIDIA_MODELS",
        "return createProvider({ id: \"nvidia\", name: \"NVIDIA\", baseUrl: \"https://integrate.api.nvidia.com/v1\", auth: { apiKey: envApiKeyAuth(\"NVIDIA API key\", [\"NVIDIA_API_KEY\"]) }, models: Object.values(NVIDIA_MODELS), api: openAICompletionsApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_nvidia_provider_blocker() {
        match nvidia_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("NVIDIA_MODELS"));
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
    fn preserves_nvidia_provider_constants() {
        assert_eq!(NVIDIA_PROVIDER_ID, "nvidia");
        assert_eq!(NVIDIA_PROVIDER_NAME, "NVIDIA");
        assert_eq!(NVIDIA_BASE_URL, "https://integrate.api.nvidia.com/v1");
        assert_eq!(NVIDIA_API, "openai-completions");
        assert_eq!(NVIDIA_API_KEY_AUTH_NAME, "NVIDIA API key");
        assert_eq!(NVIDIA_API_KEY_ENV_VARS, &["NVIDIA_API_KEY"]);
    }
}
