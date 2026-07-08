//! Lazy OpenAI Completions API entry point ported from Pi.

use zedflow_core::placeholders;

use super::lazy::{ProviderStreams, lazy_api};

/// Returns the lazy OpenAI Completions provider streams.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/api/openai-completions.ts`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `load ./openai-completions.ts on first stream or streamSimple call through lazyApi; loaded module must provide ProviderStreams, and load failures must surface as assistant error events without live provider calls during construction`.
/// Replacement decision needed before production use.
#[must_use]
pub fn open_ai_completions_api() -> ProviderStreams {
    lazy_api(|| {
        placeholders::unsupported(
            "references/pi/packages/ai/src/api/openai-completions.ts",
            "load ./openai-completions.ts on first stream or streamSimple call through lazyApi; loaded module must provide ProviderStreams, and load failures must surface as assistant error events without live provider calls during construction",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::super::lazy::{AssistantMessageEvent, Context, Model};
    use super::*;

    #[test]
    fn construction_is_lazy_and_stream_reports_placeholder_without_network() {
        let provider = open_ai_completions_api();
        let model = Model::new("gpt-test", "openai-completions", "openai");

        let stream = provider.stream(&model, &Context, None);

        assert!(stream.is_done());
        let Some(AssistantMessageEvent::Error { error, .. }) = stream.events().front() else {
            panic!("expected lazy load error event");
        };
        assert_eq!(error.api, "openai-completions");
        assert_eq!(error.provider, "openai");
        assert_eq!(error.model, "gpt-test");
        assert!(
            error
                .error_message
                .as_deref()
                .is_some_and(|message| message.contains("openai-completions.ts"))
        );
    }
}
