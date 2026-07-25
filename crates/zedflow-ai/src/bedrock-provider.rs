//! Bedrock provider module ported from Pi's `packages/ai/src/bedrock-provider.ts`.

use crate::api::bedrock_converse_stream::{
    AssistantMessageEventStream, BedrockOptions, Context, Model, stream, stream_simple,
};
use crate::error::Result;

/// Bedrock stream function signature exported by Pi's Bedrock provider module.
pub type BedrockStreamFn =
    fn(&Model, &Context, Option<&BedrockOptions>) -> Result<AssistantMessageEventStream>;

/// Provider module containing Bedrock stream entry points.
#[derive(Debug, Clone, Copy)]
pub struct BedrockProviderModule {
    /// Bedrock Converse Stream API entry point.
    pub stream: BedrockStreamFn,
    /// Simple-options wrapper for Bedrock Converse Stream.
    pub stream_simple: BedrockStreamFn,
}

/// Pi's `bedrockProviderModule` export: `{ stream, streamSimple }`.
pub const BEDROCK_PROVIDER_MODULE: BedrockProviderModule = BedrockProviderModule {
    stream,
    stream_simple,
};

/// Returns Pi's Bedrock provider module export.
#[must_use]
pub const fn bedrock_provider_module() -> BedrockProviderModule {
    BEDROCK_PROVIDER_MODULE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn model() -> Model {
        Model {
            id: "anthropic.claude-sonnet-4-6".to_string(),
            provider: "amazon-bedrock".to_string(),
            name: None,
            base_url: None,
            max_tokens: 4096,
            reasoning: true,
            thinking_level_map: HashMap::new(),
        }
    }

    #[test]
    fn provider_module_exports_bedrock_stream_functions_without_network() {
        let module = bedrock_provider_module();
        let context = Context;

        (module.stream)(&model(), &context, None).expect("stream function should be wired");
        (module.stream_simple)(&model(), &context, None)
            .expect("stream_simple function should be wired");
    }
}
