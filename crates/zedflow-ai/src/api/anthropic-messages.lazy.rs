//! Lazy Anthropic Messages API entrypoint ported from Pi.

use zedflow_core::{error::Result, placeholders};

/// Placeholder for Pi's `ProviderStreams` contract used by lazy API factories.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts ProviderStreams`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `expose stream and streamSimple functions returning AssistantMessageEventStream values for provider API implementations`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderStreams;

/// Returns the lazy Anthropic Messages provider streams.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/api/lazy.ts` and `./anthropic-messages.ts`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./anthropic-messages.ts; module import failures surface as assistant error events without live provider calls during construction`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the shared `ProviderStreams` and lazy stream machinery are ported.
#[must_use]
pub fn anthropic_messages_api() -> Result<ProviderStreams> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/api/lazy.ts and ./anthropic-messages.ts",
        "return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./anthropic-messages.ts; module import failures surface as assistant error events without live provider calls during construction",
    )
}
