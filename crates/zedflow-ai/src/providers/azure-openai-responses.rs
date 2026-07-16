//! Azure OpenAI Responses provider ported from Pi's `packages/ai/src/providers/azure-openai-responses.ts`.

use crate::models::{Provider, ProviderApi};
use crate::providers::static_catalog::static_provider;

/// Azure OpenAI Responses provider id used by Pi.
pub const AZURE_OPENAI_RESPONSES_PROVIDER_ID: &str = "azure-openai-responses";

/// Azure OpenAI Responses display name used by Pi.
pub const AZURE_OPENAI_RESPONSES_PROVIDER_NAME: &str = "Azure OpenAI";

/// Azure OpenAI Responses API-key auth prompt label used by Pi.
pub const AZURE_OPENAI_RESPONSES_API_KEY_AUTH_NAME: &str = "Azure OpenAI API key";

/// Environment variables checked for Azure OpenAI Responses API-key auth, in Pi precedence order.
pub const AZURE_OPENAI_RESPONSES_API_KEY_ENV_VARS: &[&str] = &["AZURE_OPENAI_API_KEY"];

/// Creates the azure-openai-responses provider from the static Rust model catalog.
#[must_use]
pub fn azure_openai_responses_provider() -> Provider {
    let mut provider = static_provider(
        AZURE_OPENAI_RESPONSES_PROVIDER_ID,
        AZURE_OPENAI_RESPONSES_PROVIDER_NAME,
        crate::providers::azure_openai_responses_models::azure_openai_responses_models(),
    );
    provider.api =
        ProviderApi::Single(crate::api::azure_openai_responses_lazy::azure_open_ai_responses_api());
    provider
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_from_static_catalog() {
        let provider = azure_openai_responses_provider();
        assert_eq!(provider.id, AZURE_OPENAI_RESPONSES_PROVIDER_ID);
        assert_eq!(provider.name, AZURE_OPENAI_RESPONSES_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
    }
}
