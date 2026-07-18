//! Static OpenAI Codex Responses API entry point ported from Pi.

use crate::types::ProviderStreams;

/// Returns the OpenAI Codex Responses provider streams.
#[must_use]
pub fn open_ai_codex_responses_api() -> ProviderStreams {
    super::openai_codex_responses::provider_streams()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use futures::executor::block_on;

    #[test]
    fn uses_the_real_codex_transport() {
        let api = open_ai_codex_responses_api();
        let mut stream = (api.stream)(
            &crate::types::Model {
                id: "gpt-5.1-codex".into(),
                api: "openai-codex-responses".into(),
                provider: "missing-codex-test".into(),
                ..crate::types::Model::default()
            },
            &crate::types::Context::default(),
            None,
        );
        let event = block_on(stream.next()).expect("terminal error");
        let crate::types::AssistantMessageEvent::Error { error, .. } = event else {
            panic!("expected terminal error")
        };
        assert!(
            error
                .error_message
                .is_some_and(|message| message.contains("no API key for provider"))
        );
    }
}
