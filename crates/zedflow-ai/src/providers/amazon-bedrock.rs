//! Amazon Bedrock provider ported from Pi's `packages/ai/src/providers/amazon-bedrock.ts`.

use std::sync::Arc;

use crate::models::{
    AssistantMessageEventStream, CreateProviderOptions, Model, Provider, StreamOptions,
    create_provider,
};
use crate::providers::amazon_bedrock_models::amazon_bedrock_models;

/// Creates the Amazon Bedrock provider.
#[must_use]
pub fn amazon_bedrock_provider() -> Provider {
    create_provider(CreateProviderOptions {
        id: "amazon-bedrock".into(),
        name: Some("Amazon Bedrock".into()),
        models: amazon_bedrock_models(),
        refresh_models: None,
        stream: Arc::new(|_model: &Model, _options: Option<&StreamOptions>| {
            AssistantMessageEventStream::new()
        }),
    })
}

/// Environment variables that can satisfy Bedrock ambient auth.
pub const BEDROCK_AUTH_ENV: &[&str] = &[
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_PROFILE",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_bedrock_provider() {
        let provider = amazon_bedrock_provider();
        assert_eq!(provider.id, "amazon-bedrock");
        assert!(!provider.get_models().is_empty());
    }
}
