//! Static Bedrock Converse Stream API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the Bedrock Converse Stream provider streams.
#[must_use]
pub fn bedrock_converse_stream_api() -> ProviderStreams {
    super::bedrock_converse_stream::provider_streams()
}
