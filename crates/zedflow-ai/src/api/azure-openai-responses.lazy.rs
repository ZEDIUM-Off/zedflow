//! Static Azure OpenAI Responses API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the Azure OpenAI Responses provider streams.
#[must_use]
pub fn azure_open_ai_responses_api() -> ProviderStreams {
    super::azure_openai_responses::provider_streams()
}
