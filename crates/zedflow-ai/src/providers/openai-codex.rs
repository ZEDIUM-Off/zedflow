//! OpenAI Codex provider factory ported from Pi's `packages/ai/src/providers/openai-codex.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// OpenAI Codex provider id used by Pi.
pub const OPENAI_CODEX_PROVIDER_ID: &str = "openai-codex";

/// OpenAI Codex display name used by Pi.
pub const OPENAI_CODEX_PROVIDER_NAME: &str = "OpenAI Codex";

/// OpenAI Codex backend API base URL used by Pi.
pub const OPENAI_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";

/// OpenAI Codex OAuth display name used by Pi.
pub const OPENAI_CODEX_OAUTH_NAME: &str = "OpenAI (ChatGPT Plus/Pro)";

/// OpenAI Codex stream API id used by Pi models.
pub const OPENAI_CODEX_API: &str = "openai-codex-responses";

/// Creates Pi's OpenAI Codex provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts lazyOAuth, references/pi/packages/ai/src/utils/oauth/load.ts loadOpenAICodexOAuth, references/pi/packages/ai/src/api/openai-codex-responses.lazy.ts openAICodexResponsesApi, references/pi/packages/ai/src/providers/openai-codex.models.ts OPENAI_CODEX_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "openai-codex", name: "OpenAI Codex", baseUrl: "https://chatgpt.com/backend-api", auth: { oauth: lazyOAuth({ name: "OpenAI (ChatGPT Plus/Pro)", load: loadOpenAICodexOAuth }) }, models: Object.values(OPENAI_CODEX_MODELS), api: openAICodexResponsesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract,
/// OpenAI Codex OAuth loader, and OpenAI Codex model catalog are available in Rust.
#[must_use]
pub fn openai_codex_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts lazyOAuth, references/pi/packages/ai/src/utils/oauth/load.ts loadOpenAICodexOAuth, references/pi/packages/ai/src/api/openai-codex-responses.lazy.ts openAICodexResponsesApi, references/pi/packages/ai/src/providers/openai-codex.models.ts OPENAI_CODEX_MODELS",
        "return createProvider({ id: \"openai-codex\", name: \"OpenAI Codex\", baseUrl: \"https://chatgpt.com/backend-api\", auth: { oauth: lazyOAuth({ name: \"OpenAI (ChatGPT Plus/Pro)\", load: loadOpenAICodexOAuth }) }, models: Object.values(OPENAI_CODEX_MODELS), api: openAICodexResponsesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_openai_codex_provider_blocker() {
        match openai_codex_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("lazyOAuth"));
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("loadOpenAICodexOAuth")
                );
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("OPENAI_CODEX_MODELS")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICodexResponsesApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_openai_codex_provider_constants() {
        assert_eq!(OPENAI_CODEX_PROVIDER_ID, "openai-codex");
        assert_eq!(OPENAI_CODEX_PROVIDER_NAME, "OpenAI Codex");
        assert_eq!(OPENAI_CODEX_BASE_URL, "https://chatgpt.com/backend-api");
        assert_eq!(OPENAI_CODEX_OAUTH_NAME, "OpenAI (ChatGPT Plus/Pro)");
        assert_eq!(OPENAI_CODEX_API, "openai-codex-responses");
    }
}
