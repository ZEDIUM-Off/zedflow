//! Static OpenAI Responses API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the OpenAI Responses provider streams.
#[must_use]
pub fn open_ai_responses_api() -> ProviderStreams {
    super::openai_responses::provider_streams()
}
