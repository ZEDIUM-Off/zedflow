//! OpenAI provider factory ported from Pi's `packages/ai/src/providers/openai.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// OpenAI provider id used by Pi.
pub const OPENAI_PROVIDER_ID: &str = "openai";

/// OpenAI display name used by Pi.
pub const OPENAI_PROVIDER_NAME: &str = "OpenAI";

/// OpenAI API base URL used by Pi.
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// OpenAI stream API id used by Pi models.
pub const OPENAI_API: &str = "openai-responses";

/// OpenAI API-key auth prompt label used by Pi.
pub const OPENAI_API_KEY_AUTH_NAME: &str = "OpenAI API key";

/// Environment variables checked for OpenAI API-key auth, in Pi precedence order.
pub const OPENAI_API_KEY_ENV_VARS: &[&str] = &["OPENAI_API_KEY"];

/// Creates Pi's OpenAI provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/openai.models.ts OPENAI_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "openai", name: "OpenAI", baseUrl: "https://api.openai.com/v1", auth: { apiKey: envApiKeyAuth("OpenAI API key", ["OPENAI_API_KEY"]) }, models: Object.values(OPENAI_MODELS), api: openAIResponsesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract can
/// construct Pi's `createProvider` output from `OPENAI_MODELS` in Rust.
#[must_use]
pub fn openai_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/openai.models.ts OPENAI_MODELS",
        "return createProvider({ id: \"openai\", name: \"OpenAI\", baseUrl: \"https://api.openai.com/v1\", auth: { apiKey: envApiKeyAuth(\"OpenAI API key\", [\"OPENAI_API_KEY\"]) }, models: Object.values(OPENAI_MODELS), api: openAIResponsesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_openai_provider_blocker() {
        match openai_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("OPENAI_MODELS"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAIResponsesApi")
                );
                assert!(placeholder.required_behavior().contains("OPENAI_API_KEY"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_openai_provider_constants() {
        assert_eq!(OPENAI_PROVIDER_ID, "openai");
        assert_eq!(OPENAI_PROVIDER_NAME, "OpenAI");
        assert_eq!(OPENAI_BASE_URL, "https://api.openai.com/v1");
        assert_eq!(OPENAI_API, "openai-responses");
        assert_eq!(OPENAI_API_KEY_AUTH_NAME, "OpenAI API key");
        assert_eq!(OPENAI_API_KEY_ENV_VARS, &["OPENAI_API_KEY"]);
    }
}
