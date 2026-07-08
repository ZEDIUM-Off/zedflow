//! Azure OpenAI Responses provider ported from Pi's `packages/ai/src/providers/azure-openai-responses.ts`.

use std::sync::Arc;

use crate::models::{
    AssistantMessageEventStream, CreateProviderOptions, Model, Provider, StreamOptions,
    create_provider,
};
use crate::providers::azure_openai_responses_models::azure_openai_responses_models;

/// Azure OpenAI Responses provider id used by Pi.
pub const AZURE_OPENAI_RESPONSES_PROVIDER_ID: &str = "azure-openai-responses";

/// Azure OpenAI Responses display name used by Pi.
pub const AZURE_OPENAI_RESPONSES_PROVIDER_NAME: &str = "Azure OpenAI";

/// Azure OpenAI Responses API-key auth prompt label used by Pi.
pub const AZURE_OPENAI_RESPONSES_API_KEY_AUTH_NAME: &str = "Azure OpenAI API key";

/// Environment variables checked for Azure OpenAI Responses API-key auth, in Pi precedence order.
pub const AZURE_OPENAI_RESPONSES_API_KEY_ENV_VARS: &[&str] = &["AZURE_OPENAI_API_KEY"];

/// Creates Pi's Azure OpenAI Responses provider.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/models.ts Provider/createProvider, references/pi/packages/ai/src/auth/helpers.ts envApiKeyAuth, references/pi/packages/ai/src/api/azure-openai-responses.lazy.ts azureOpenAIResponsesApi`.
/// Reason: the current Rust `Provider` shape has model catalog and stream hooks, but no auth/API fields.
/// Required behavior: `return createProvider({ id: "azure-openai-responses", name: "Azure OpenAI", auth: { apiKey: envApiKeyAuth("Azure OpenAI API key", ["AZURE_OPENAI_API_KEY"]) }, models: Object.values(AZURE_OPENAI_RESPONSES_MODELS), api: azureOpenAIResponsesApi() })`.
/// Replacement decision needed before production streaming/auth use.
#[must_use]
pub fn azure_openai_responses_provider() -> Provider {
    create_provider(CreateProviderOptions {
        id: AZURE_OPENAI_RESPONSES_PROVIDER_ID.into(),
        name: Some(AZURE_OPENAI_RESPONSES_PROVIDER_NAME.into()),
        models: azure_openai_responses_models(),
        refresh_models: None,
        stream: Arc::new(|_model: &Model, _options: Option<&StreamOptions>| {
            AssistantMessageEventStream::new()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_azure_openai_responses_provider() {
        let provider = azure_openai_responses_provider();
        assert_eq!(provider.id, AZURE_OPENAI_RESPONSES_PROVIDER_ID);
        assert_eq!(provider.name, AZURE_OPENAI_RESPONSES_PROVIDER_NAME);
        assert_eq!(provider.get_models().len(), 42);
    }

    #[test]
    fn preserves_azure_openai_responses_auth_constants() {
        assert_eq!(
            AZURE_OPENAI_RESPONSES_API_KEY_AUTH_NAME,
            "Azure OpenAI API key"
        );
        assert_eq!(
            AZURE_OPENAI_RESPONSES_API_KEY_ENV_VARS,
            &["AZURE_OPENAI_API_KEY"]
        );
    }
}
