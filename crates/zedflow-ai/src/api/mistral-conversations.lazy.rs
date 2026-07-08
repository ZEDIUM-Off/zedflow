//! Lazy Mistral Conversations API entry point ported from Pi.

use zedflow_core::error::Result;

use super::mistral_conversations::{
    self, AssistantMessageEventStream, Context, MistralOptions, Model, SimpleStreamOptions,
};

/// Mistral provider stream functions exposed by the lazy API factory.
#[derive(Debug, Clone, Copy)]
pub struct ProviderStreams {
    /// Full Mistral stream function.
    pub stream: fn(
        &Model,
        &Context,
        Option<&MistralOptions>,
    ) -> mistral_conversations::Result<AssistantMessageEventStream>,
    /// Simple-options Mistral stream function.
    pub stream_simple: fn(
        &Model,
        &Context,
        Option<&SimpleStreamOptions>,
    ) -> mistral_conversations::Result<AssistantMessageEventStream>,
}

/// Returns the lazy Mistral Conversations provider streams.
#[must_use]
pub fn mistral_conversations_api() -> Result<ProviderStreams> {
    Ok(ProviderStreams {
        stream: mistral_conversations::stream,
        stream_simple: mistral_conversations::stream_simple,
    })
}
