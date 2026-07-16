//! Static Google Vertex API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the Google Vertex provider streams.
#[must_use]
pub fn google_vertex_api() -> ProviderStreams {
    super::google_vertex::provider_streams()
}
