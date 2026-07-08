//! Lazy OpenAI Codex Responses API entry point ported from Pi.

use crate::api::lazy::{ProviderStreams, lazy_api};
use zedflow_core::placeholders;

/// Returns the lazy OpenAI Codex Responses provider streams.
///
/// PORT PLACEHOLDER:
/// Original dependency: `JavaScript dynamic import()` and `./openai-codex-responses.ts`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./openai-codex-responses.ts; module import failures surface as assistant error events without live provider calls during construction`.
/// Replacement decision needed before production use.
#[must_use]
pub fn open_ai_codex_responses_api() -> ProviderStreams {
    lazy_api(|| {
        Err::<ProviderStreams, _>(placeholders::error(
            "JavaScript dynamic import() and ./openai-codex-responses.ts",
            "return ProviderStreams whose stream and streamSimple synchronously return streams backed by lazy loading ./openai-codex-responses.ts; module import failures surface as assistant error events without live provider calls during construction",
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::lazy::{AssistantMessageEvent, Context, Model};

    #[test]
    fn construction_is_lazy_and_stream_reports_missing_module() {
        let api = open_ai_codex_responses_api();
        let stream = api.stream(
            &Model::new("codex", "openai-codex-responses", "openai-codex"),
            &Context,
            None,
        );

        let Some(AssistantMessageEvent::Error { error, .. }) = stream.events().front() else {
            panic!("expected dynamic import placeholder error");
        };
        assert!(
            error
                .error_message
                .as_deref()
                .is_some_and(|message| message.contains("openai-codex-responses.ts"))
        );
    }
}
