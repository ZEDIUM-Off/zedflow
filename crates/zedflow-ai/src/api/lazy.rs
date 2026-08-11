//! Helpers for deferred provider stream construction.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageRole, ErrorStopReason, Model,
    ProviderStreams, SimpleStreamOptions, StopReason, StreamOptions, Usage,
};
use crate::utils::event_stream::AssistantMessageEventStream;

/// Returns the provider stream produced by setup, or a canonical terminal error stream.
///
/// Setup is synchronous in Rust, so a successful setup returns the actual provider stream rather
/// than forwarding or materializing it. Delayed producer events remain observable incrementally.
#[must_use]
pub fn lazy_stream<E>(
    model: &Model,
    setup: impl FnOnce() -> Result<AssistantMessageEventStream, E>,
) -> AssistantMessageEventStream
where
    E: ToString,
{
    setup().unwrap_or_else(|error| terminal_error_stream(model, error))
}

/// Wraps a lazily loaded canonical provider implementation.
///
/// This mirrors Pi's `lazyApi`: each stream call invokes `load`, while the concrete Rust provider
/// stream is returned unchanged so its events remain incremental. Rust provider modules are linked
/// statically; callers that need module-level caching can use their own `OnceLock`.
#[must_use]
pub fn lazy_api<E>(
    load: impl Fn() -> Result<ProviderStreams, E> + Send + Sync + 'static,
) -> ProviderStreams
where
    E: ToString,
{
    let load = Arc::new(load);
    let stream_load = Arc::clone(&load);
    let simple_load = Arc::clone(&load);

    ProviderStreams {
        stream: Arc::new(move |model, context, options| {
            lazy_stream(model, || {
                (stream_load)().map(|streams| (streams.stream)(model, context, options))
            })
        }),
        stream_simple: Arc::new(move |model, context, options| {
            lazy_stream(model, || {
                (simple_load)().map(|streams| (streams.stream_simple)(model, context, options))
            })
        }),
    }
}

/// Creates an immediately settled canonical terminal error stream.
#[must_use]
pub fn terminal_error_stream(model: &Model, error: impl ToString) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let message = AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::Error,
        error_message: Some(error.to_string()),
        timestamp: unix_timestamp_millis(),
    };
    stream.push(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Error,
        error: message,
    });
    stream
}

fn unix_timestamp_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::StreamExt;
    use futures::executor::block_on;

    use super::*;
    use crate::types::{
        AssistantContentBlock, Context, DoneStopReason, TextContent, TextContentType,
    };

    fn model() -> Model {
        Model {
            id: "model-id".into(),
            api: "api".into(),
            provider: "provider".into(),
            ..Model::default()
        }
    }

    fn message(stop_reason: StopReason) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "done".into(),
                text_signature: None,
            })],
            api: "api".into(),
            provider: "provider".into(),
            model: "model-id".into(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason,
            error_message: None,
            timestamp: 0,
        }
    }

    #[test]
    fn setup_failure_emits_one_terminal_error_and_settles_same_message() {
        block_on(async {
            let mut stream = lazy_stream(&model(), || {
                Err::<AssistantMessageEventStream, _>("load failed")
            });
            let event = stream.next().await.expect("terminal event");
            let AssistantMessageEvent::Error { error, .. } = event else {
                panic!("expected error event");
            };
            assert_eq!(error.error_message.as_deref(), Some("load failed"));
            assert_eq!(stream.next().await, None);
            assert_eq!(stream.result().await, error);
        });
    }

    #[test]
    fn lazy_api_loads_per_call_and_returns_actual_stream_with_delayed_incremental_delivery() {
        block_on(async {
            let loads = Arc::new(AtomicUsize::new(0));
            let load_count = Arc::clone(&loads);
            let release = Arc::new(std::sync::Barrier::new(2));
            let producer_release = Arc::clone(&release);
            let provider = lazy_api(move || {
                load_count.fetch_add(1, Ordering::SeqCst);
                let producer_release = Arc::clone(&producer_release);
                Ok::<_, String>(ProviderStreams {
                    stream: Arc::new(move |_model, _context, _options| {
                        let stream = AssistantMessageEventStream::new();
                        let producer = stream.clone();
                        let release = Arc::clone(&producer_release);
                        std::thread::spawn(move || {
                            release.wait();
                            producer.push(AssistantMessageEvent::Start {
                                partial: message(StopReason::Stop).into(),
                            });
                            producer.push(AssistantMessageEvent::Done {
                                reason: DoneStopReason::Stop,
                                message: message(StopReason::Stop),
                            });
                        });
                        stream
                    }),
                    stream_simple: Arc::new(|_model, _context, _options| {
                        AssistantMessageEventStream::new()
                    }),
                })
            });

            let mut stream: AssistantMessageEventStream =
                (provider.stream)(&model(), &Context::default(), None);
            assert!(!stream.is_done());
            release.wait();
            assert!(matches!(
                stream.next().await,
                Some(AssistantMessageEvent::Start { .. })
            ));
            assert!(matches!(
                stream.next().await,
                Some(AssistantMessageEvent::Done { .. })
            ));
            assert_eq!(stream.next().await, None);
            assert_eq!(stream.result().await, message(StopReason::Stop));
            let _ = (provider.stream_simple)(&model(), &Context::default(), None);
            assert_eq!(loads.load(Ordering::SeqCst), 2);
        });
    }
}
