//! Anthropic provider factory ported from Pi's `packages/ai/src/providers/anthropic.ts`.

use zedflow_core::{error::Result, placeholders};

use crate::models::Provider;

/// Anthropic provider id used by Pi.
pub const ANTHROPIC_PROVIDER_ID: &str = "anthropic";

/// Anthropic display name used by Pi.
pub const ANTHROPIC_PROVIDER_NAME: &str = "Anthropic";

/// Anthropic API base URL used by Pi.
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

/// Anthropic API-key auth prompt label used by Pi.
pub const ANTHROPIC_API_KEY_AUTH_NAME: &str = "Anthropic API key";

/// Anthropic OAuth prompt label used by Pi.
pub const ANTHROPIC_OAUTH_NAME: &str = "Anthropic (Claude Pro/Max)";

/// Environment variables checked for Anthropic API-key auth, in Pi precedence order.
pub const ANTHROPIC_API_KEY_ENV_VARS: &[&str] = &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"];

/// Creates Pi's Anthropic provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts, references/pi/packages/ai/src/utils/oauth/load.ts loadAnthropicOAuth`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return createProvider({ id: "anthropic", name: "Anthropic", baseUrl: "https://api.anthropic.com", auth: { apiKey: envApiKeyAuth("Anthropic API key", ["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]), oauth: lazyOAuth({ name: "Anthropic (Claude Pro/Max)", load: loadAnthropicOAuth }) }, models: Object.values(ANTHROPIC_MODELS), api: anthropicMessagesApi() })`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared provider auth/base URL/API stream contract and
/// Anthropic OAuth loader are available in Rust.
pub fn anthropic_provider() -> Result<Provider> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/api/anthropic-messages.lazy.ts, references/pi/packages/ai/src/utils/oauth/load.ts loadAnthropicOAuth",
        "return createProvider({ id: \"anthropic\", name: \"Anthropic\", baseUrl: \"https://api.anthropic.com\", auth: { apiKey: envApiKeyAuth(\"Anthropic API key\", [\"ANTHROPIC_OAUTH_TOKEN\", \"ANTHROPIC_API_KEY\"]), oauth: lazyOAuth({ name: \"Anthropic (Claude Pro/Max)\", load: loadAnthropicOAuth }) }, models: Object.values(ANTHROPIC_MODELS), api: anthropicMessagesApi() })",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zedflow_core::error::Error;

    #[test]
    fn documents_anthropic_provider_blocker() {
        let err = anthropic_provider().expect_err("provider creation is intentionally blocked");
        match err {
            Error::PortPlaceholder(placeholder) => {
                assert!(placeholder.original_dependency().contains("createProvider"));
                assert!(
                    placeholder
                        .required_behavior()
                        .contains("ANTHROPIC_OAUTH_TOKEN")
                );
            }
            _ => panic!("unexpected provider error: {err:?}"),
        }
    }

    #[test]
    fn preserves_anthropic_auth_precedence_constants() {
        assert_eq!(ANTHROPIC_PROVIDER_ID, "anthropic");
        assert_eq!(ANTHROPIC_BASE_URL, "https://api.anthropic.com");
        assert_eq!(
            ANTHROPIC_API_KEY_ENV_VARS,
            &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]
        );
    }
}
