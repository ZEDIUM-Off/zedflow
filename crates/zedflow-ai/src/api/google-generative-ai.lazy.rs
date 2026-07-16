//! Static Google Generative AI API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the Google Generative AI provider streams.
#[must_use]
pub fn google_generative_ai_api() -> ProviderStreams {
    super::google_generative_ai::provider_streams()
}
