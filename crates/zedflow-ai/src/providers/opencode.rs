//! OpenCode Zen provider factory ported from Pi's `packages/ai/src/providers/opencode.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// OpenCode Zen provider id used by Pi.
pub const OPENCODE_PROVIDER_ID: &str = "opencode";

/// OpenCode Zen display name used by Pi.
pub const OPENCODE_PROVIDER_NAME: &str = "OpenCode Zen";

/// OpenCode Zen API-key auth prompt label used by Pi.
pub const OPENCODE_API_KEY_AUTH_NAME: &str = "OpenCode API key";

/// Environment variables checked for OpenCode Zen API-key auth, in Pi precedence order.
pub const OPENCODE_API_KEY_ENV_VARS: &[&str] = &["OPENCODE_API_KEY"];

/// OpenCode Zen stream API ids used by Pi models.
pub const OPENCODE_APIS: &[&str] = &[
    "anthropic-messages",
    "google-generative-ai",
    "openai-completions",
    "openai-responses",
];

/// Creates Pi's OpenCode Zen provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/google-generative-ai.lazy.ts googleGenerativeAIApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/opencode.models.ts OPENCODE_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "opencode", name: "OpenCode Zen", auth: { apiKey: envApiKeyAuth("OpenCode API key", ["OPENCODE_API_KEY"]) }, models: Object.values(OPENCODE_MODELS), api: { "anthropic-messages": anthropicMessagesApi(), "google-generative-ai": googleGenerativeAIApi(), "openai-completions": openAICompletionsApi(), "openai-responses": openAIResponsesApi() } })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/API stream contract and
/// OpenCode Zen model catalog are available in Rust.
#[must_use]
pub fn opencode_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/google-generative-ai.lazy.ts googleGenerativeAIApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/opencode.models.ts OPENCODE_MODELS",
        "return createProvider({ id: \"opencode\", name: \"OpenCode Zen\", auth: { apiKey: envApiKeyAuth(\"OpenCode API key\", [\"OPENCODE_API_KEY\"]) }, models: Object.values(OPENCODE_MODELS), api: { \"anthropic-messages\": anthropicMessagesApi(), \"google-generative-ai\": googleGenerativeAIApi(), \"openai-completions\": openAICompletionsApi(), \"openai-responses\": openAIResponsesApi() } })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_opencode_provider_blocker() {
        match opencode_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("OPENCODE_MODELS")
                );
                assert!(placeholder.original_dependency().contains("envApiKeyAuth"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("anthropicMessagesApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("googleGenerativeAIApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAICompletionsApi")
                );
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("openAIResponsesApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_opencode_provider_constants() {
        assert_eq!(OPENCODE_PROVIDER_ID, "opencode");
        assert_eq!(OPENCODE_PROVIDER_NAME, "OpenCode Zen");
        assert_eq!(OPENCODE_API_KEY_AUTH_NAME, "OpenCode API key");
        assert_eq!(OPENCODE_API_KEY_ENV_VARS, &["OPENCODE_API_KEY"]);
        assert_eq!(
            OPENCODE_APIS,
            &[
                "anthropic-messages",
                "google-generative-ai",
                "openai-completions",
                "openai-responses"
            ]
        );
    }
}
