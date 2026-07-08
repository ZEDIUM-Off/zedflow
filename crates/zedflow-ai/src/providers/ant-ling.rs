//! Ant Ling provider ported from Pi's `packages/ai/src/providers/ant-ling.ts`.

use std::sync::Arc;

use crate::models::{
    AssistantMessageEventStream, CreateProviderOptions, Model, Provider, StreamOptions,
    create_provider,
};
use crate::providers::ant_ling_models::ant_ling_models;

/// Ant Ling OpenAI-compatible base URL.
pub const ANT_LING_BASE_URL: &str = "https://api.ant-ling.com/v1";

/// Env var used for Ant Ling API keys.
pub const ANT_LING_API_KEY_ENV: &str = "ANT_LING_API_KEY";

/// Creates the Ant Ling provider.
#[must_use]
pub fn ant_ling_provider() -> Provider {
    create_provider(CreateProviderOptions {
        id: "ant-ling".into(),
        name: Some("Ant Ling".into()),
        models: ant_ling_models(),
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
    fn builds_ant_ling_provider() {
        let provider = ant_ling_provider();
        assert_eq!(provider.id, "ant-ling");
        assert_eq!(provider.get_models().len(), 3);
    }
}
