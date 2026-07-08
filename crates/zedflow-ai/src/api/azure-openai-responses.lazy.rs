//! Lazy Azure OpenAI Responses API entry point ported from Pi.

use zedflow_core::{error::Result, placeholders};

/// Placeholder for Pi's `ProviderStreams` contract used by lazy API factories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStreams;

/// Returns the lazy Azure OpenAI Responses provider streams.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/api/lazy.ts` and `./azure-openai-responses.ts`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./azure-openai-responses.ts; module import failures surface as assistant error events without live provider calls during construction`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared `ProviderStreams` and lazy stream machinery are ported.
#[must_use]
pub fn azure_open_ai_responses_api() -> Result<ProviderStreams> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/api/lazy.ts and ./azure-openai-responses.ts",
        "return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./azure-openai-responses.ts; module import failures surface as assistant error events without live provider calls during construction",
    )
}
