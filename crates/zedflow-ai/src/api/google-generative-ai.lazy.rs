//! Lazy Google Generative AI API entry point ported from Pi.

use zedflow_core::error::Result;

/// Provider streams marker for the Google Generative AI implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStreams;

/// Returns the Google Generative AI provider streams.
#[must_use]
pub fn google_generative_ai_api() -> Result<ProviderStreams> {
    Ok(ProviderStreams)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_google_streams_without_dynamic_import() {
        assert_eq!(
            google_generative_ai_api().expect("streams"),
            ProviderStreams
        );
    }
}
