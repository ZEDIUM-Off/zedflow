//! Static OpenAI Codex Responses API entry point ported from Pi.

use crate::types::ProviderStreams;

use super::lazy::terminal_error_api;

/// Returns the OpenAI Codex Responses provider streams.
#[must_use]
pub fn open_ai_codex_responses_api() -> ProviderStreams {
    terminal_error_api("openai-codex-responses")
}
