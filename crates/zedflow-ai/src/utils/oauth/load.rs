//! Lazy OAuth module loaders ported from Pi's `packages/ai/src/utils/oauth/load.ts`.

use std::sync::Arc;

use crate::auth::types::{AuthResult, OAuthAuth};
use crate::utils::oauth::anthropic::AnthropicOAuth;
use crate::utils::oauth::github_copilot::GitHubCopilotOAuth;
use crate::utils::oauth::openai_codex::OpenAiCodexOAuth;

/// Result returned by OAuth loader functions.
pub type OAuthLoadResult = AuthResult<Arc<dyn OAuthAuth>>;

/// Loads the Anthropic OAuth auth object.
///
/// Mirrors Pi's `loadAnthropicOAuth` dynamic import without needing a runtime import in Rust.
///
/// # Errors
///
/// Reserved for parity with Pi's fallible dynamic import boundary. The current static Rust load
/// path does not fail.
#[doc(alias = "loadAnthropicOAuth")]
pub async fn load_anthropic_oauth() -> OAuthLoadResult {
    Ok(Arc::new(AnthropicOAuth))
}

/// Loads the OpenAI Codex OAuth auth object.
///
/// Mirrors Pi's `loadOpenAICodexOAuth` dynamic import without needing a runtime import in Rust.
///
/// # Errors
///
/// Reserved for parity with Pi's fallible dynamic import boundary. The current static Rust load
/// path does not fail.
#[doc(alias = "loadOpenAICodexOAuth")]
pub async fn load_openai_codex_oauth() -> OAuthLoadResult {
    Ok(Arc::new(OpenAiCodexOAuth))
}

/// Loads the GitHub Copilot OAuth auth object.
///
/// Mirrors Pi's `loadGitHubCopilotOAuth` dynamic import without needing a runtime import in Rust.
///
/// # Errors
///
/// Reserved for parity with Pi's fallible dynamic import boundary. The current static Rust load
/// path does not fail.
#[doc(alias = "loadGitHubCopilotOAuth")]
pub async fn load_github_copilot_oauth() -> OAuthLoadResult {
    Ok(Arc::new(GitHubCopilotOAuth))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use futures::executor::block_on;

    use super::*;
    use crate::auth::types::OAuthCredential;
    use crate::utils::oauth::openai_codex::OPENAI_CODEX_OAUTH_NAME;

    fn credential() -> OAuthCredential {
        OAuthCredential {
            refresh: "refresh".to_owned(),
            access: "access".to_owned(),
            expires: 0,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn loads_ported_oauth_handlers() {
        let anthropic = block_on(load_anthropic_oauth()).expect("loads Anthropic OAuth");
        assert_eq!(anthropic.name(), "Anthropic (Claude Pro/Max)");

        let github = block_on(load_github_copilot_oauth()).expect("loads GitHub Copilot OAuth");
        assert_eq!(github.name(), "GitHub Copilot");
    }

    #[test]
    fn openai_codex_loader_preserves_to_auth_and_documents_blocked_network_flows() {
        let auth = block_on(load_openai_codex_oauth()).expect("loads OpenAI Codex OAuth");
        assert_eq!(auth.name(), OPENAI_CODEX_OAUTH_NAME);

        let model_auth = block_on(auth.to_auth(&credential())).expect("to_auth is local");
        assert_eq!(model_auth.api_key.as_deref(), Some("access"));

        let error = block_on(auth.refresh(&credential())).expect_err("refresh is blocked");
        assert!(error.to_string().contains("OpenAI Codex OAuth"));
    }
}
