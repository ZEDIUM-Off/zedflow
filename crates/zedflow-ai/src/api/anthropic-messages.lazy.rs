//! Static Anthropic Messages API entrypoint ported from Pi.

use crate::types::ProviderStreams;

/// Returns the Anthropic Messages provider streams.
#[must_use]
pub fn anthropic_messages_api() -> ProviderStreams {
    super::anthropic_messages::provider_streams()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessageEvent, Context, Model};
    use futures::StreamExt;
    use futures::executor::block_on;

    #[test]
    fn construction_uses_real_stream_and_reports_missing_auth_once() {
        let provider = anthropic_messages_api();
        let mut stream = (provider.stream)(
            &Model {
                id: "claude".into(),
                api: "anthropic-messages".into(),
                provider: "anthropic".into(),
                base_url: "http://127.0.0.1:9".into(),
                ..Model::default()
            },
            &Context::default(),
            None,
        );
        assert!(matches!(
            block_on(stream.next()),
            Some(AssistantMessageEvent::Error { .. })
        ));
        assert_eq!(block_on(stream.next()), None);
    }
}
