//! GitHub Copilot provider factory ported from Pi's `packages/ai/src/providers/github-copilot.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// GitHub Copilot provider id used by Pi.
pub const GITHUB_COPILOT_PROVIDER_ID: &str = "github-copilot";

/// GitHub Copilot display name used by Pi.
pub const GITHUB_COPILOT_PROVIDER_NAME: &str = "GitHub Copilot";

/// GitHub Copilot API base URL used by Pi.
pub const GITHUB_COPILOT_BASE_URL: &str = "https://api.individual.githubcopilot.com";

/// GitHub Copilot API-key auth prompt label used by Pi.
pub const GITHUB_COPILOT_API_KEY_AUTH_NAME: &str = "GitHub Copilot token";

/// GitHub Copilot OAuth prompt label used by Pi.
pub const GITHUB_COPILOT_OAUTH_NAME: &str = "GitHub Copilot";

/// Environment variables checked for GitHub Copilot API-key auth, in Pi precedence order.
pub const GITHUB_COPILOT_API_KEY_ENV_VARS: &[&str] = &["COPILOT_GITHUB_TOKEN"];

/// API stream ids registered by Pi for GitHub Copilot.
pub const GITHUB_COPILOT_APIS: &[&str] = &[
    "anthropic-messages",
    "openai-completions",
    "openai-responses",
];

/// Creates Pi's GitHub Copilot provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth/lazyOAuth, references/pi/packages/ai/src/utils/oauth/load.ts loadGitHubCopilotOAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/github-copilot.models.ts GITHUB_COPILOT_MODELS`.
/// Reason: no Rust replacement selected yet for provider auth/OAuth/API wiring.
/// Required behavior: `return createProvider({ id: "github-copilot", name: "GitHub Copilot", baseUrl: "https://api.individual.githubcopilot.com", auth: { apiKey: envApiKeyAuth("GitHub Copilot token", ["COPILOT_GITHUB_TOKEN"]), oauth: lazyOAuth({ name: "GitHub Copilot", load: loadGitHubCopilotOAuth }) }, models: Object.values(GITHUB_COPILOT_MODELS), api: { "anthropic-messages": anthropicMessagesApi(), "openai-completions": openAICompletionsApi(), "openai-responses": openAIResponsesApi() } })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// GitHub Copilot OAuth loader are available in Rust.
#[must_use]
pub fn github_copilot_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth/lazyOAuth, references/pi/packages/ai/src/utils/oauth/load.ts loadGitHubCopilotOAuth, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts anthropicMessagesApi, references/pi/packages/ai/src/api/openai-completions.lazy.ts openAICompletionsApi, references/pi/packages/ai/src/api/openai-responses.lazy.ts openAIResponsesApi, references/pi/packages/ai/src/providers/github-copilot.models.ts GITHUB_COPILOT_MODELS",
        "return createProvider({ id: \"github-copilot\", name: \"GitHub Copilot\", baseUrl: \"https://api.individual.githubcopilot.com\", auth: { apiKey: envApiKeyAuth(\"GitHub Copilot token\", [\"COPILOT_GITHUB_TOKEN\"]), oauth: lazyOAuth({ name: \"GitHub Copilot\", load: loadGitHubCopilotOAuth }) }, models: Object.values(GITHUB_COPILOT_MODELS), api: { \"anthropic-messages\": anthropicMessagesApi(), \"openai-completions\": openAICompletionsApi(), \"openai-responses\": openAIResponsesApi() } })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_github_copilot_provider_blocker() {
        match github_copilot_provider() {
            Err(Error::PortPlaceholder(placeholder)) => {
                assert!(placeholder.original_dependency().contains("lazyOAuth"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("loadGitHubCopilotOAuth")
                );
                assert!(placeholder.required_behavior().contains("openai-responses"));
            }
            Err(err) => panic!("unexpected provider error: {err:?}"),
            Ok(_) => panic!("provider creation is intentionally blocked"),
        }
    }

    #[test]
    fn preserves_github_copilot_provider_constants() {
        assert_eq!(GITHUB_COPILOT_PROVIDER_ID, "github-copilot");
        assert_eq!(GITHUB_COPILOT_PROVIDER_NAME, "GitHub Copilot");
        assert_eq!(
            GITHUB_COPILOT_BASE_URL,
            "https://api.individual.githubcopilot.com"
        );
        assert_eq!(GITHUB_COPILOT_API_KEY_AUTH_NAME, "GitHub Copilot token");
        assert_eq!(GITHUB_COPILOT_OAUTH_NAME, "GitHub Copilot");
        assert_eq!(GITHUB_COPILOT_API_KEY_ENV_VARS, &["COPILOT_GITHUB_TOKEN"]);
        assert_eq!(
            GITHUB_COPILOT_APIS,
            &[
                "anthropic-messages",
                "openai-completions",
                "openai-responses"
            ]
        );
    }
}
