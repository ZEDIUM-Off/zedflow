//! Static Mistral Conversations API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the canonical Mistral Conversations provider streams.
#[must_use]
pub fn mistral_conversations_api() -> ProviderStreams {
    super::mistral_conversations::provider_streams()
}
