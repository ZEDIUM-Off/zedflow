//! Static OpenAI Completions API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the OpenAI Completions provider streams.
#[must_use]
pub fn open_ai_completions_api() -> ProviderStreams {
    super::openai_completions::provider_streams()
}
