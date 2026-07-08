//! Lazy Google Vertex API entry point ported from Pi.

use zedflow_core::error::Result;

/// Provider streams marker for the Google Vertex implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStreams;

/// Returns the Google Vertex provider streams.
#[must_use]
pub fn google_vertex_api() -> Result<ProviderStreams> {
    Ok(ProviderStreams)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_google_vertex_streams_without_dynamic_import() {
        assert_eq!(google_vertex_api().expect("streams"), ProviderStreams);
    }
}
