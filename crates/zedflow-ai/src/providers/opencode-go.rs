//! OpenCode Zen Go provider factory ported from Pi's `packages/ai/src/providers/opencode-go.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// OpenCode Zen Go provider id used by Pi.
pub const OPENCODE_GO_PROVIDER_ID: &str = "opencode-go";

/// OpenCode Zen Go display name used by Pi.
pub const OPENCODE_GO_PROVIDER_NAME: &str = "OpenCode Zen Go";

/// OpenCode Zen Go API-key auth prompt label used by Pi.
pub const OPENCODE_GO_API_KEY_AUTH_NAME: &str = "OpenCode API key";

/// Environment variables checked for OpenCode Zen Go API-key auth, in Pi precedence order.
pub const OPENCODE_GO_API_KEY_ENV_VARS: &[&str] = &["OPENCODE_API_KEY"];

/// OpenCode Zen Go stream API ids used by Pi models.
pub const OPENCODE_GO_APIS: &[&str] = &["anthropic-messages", "openai-completions"];

/// Creates Pi's OpenCode Zen Go provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/opencode-go.models.ts OPENCODE_GO_MODELS`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "opencode-go", name: "OpenCode Zen Go", auth: { apiKey: envApiKeyAuth("OpenCode API key", ["OPENCODE_API_KEY"]) }, models: Object.values(OPENCODE_GO_MODELS), api: { "anthropic-messages": anthropicMessagesApi(), "openai-completions": openAICompletionsApi() } })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/API stream contract and
/// OpenCode Zen Go model catalog are available in Rust.
#[must_use]
pub fn opencode_go_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/providers/opencode-go.models.ts OPENCODE_GO_MODELS",
        "return createProvider({ id: \"opencode-go\", name: \"OpenCode Zen Go\", auth: { apiKey: envApiKeyAuth(\"OpenCode API key\", [\"OPENCODE_API_KEY\"]) }, models: Object.values(OPENCODE_GO_MODELS), api: { \"anthropic-messages\": anthropicMessagesApi(), \"openai-completions\": openAICompletionsApi() } })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_opencode_go_provider_blocker() {
        match opencode_go_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(
                    placeholder
                        .original_dependency()
                        .contains("OPENCODE_GO_MODELS")
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
                        .contains("openAICompletionsApi")
                );
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_opencode_go_provider_constants() {
        assert_eq!(OPENCODE_GO_PROVIDER_ID, "opencode-go");
        assert_eq!(OPENCODE_GO_PROVIDER_NAME, "OpenCode Zen Go");
        assert_eq!(OPENCODE_GO_API_KEY_AUTH_NAME, "OpenCode API key");
        assert_eq!(OPENCODE_GO_API_KEY_ENV_VARS, &["OPENCODE_API_KEY"]);
        assert_eq!(
            OPENCODE_GO_APIS,
            &["anthropic-messages", "openai-completions"]
        );
    }
}
