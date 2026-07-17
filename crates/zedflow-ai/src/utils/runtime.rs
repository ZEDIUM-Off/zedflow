//! Crate-private supervised worker support for stream producers.

use std::any::Any;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use futures::FutureExt;

use crate::types::AssistantMessageEventStream;

#[derive(Clone)]
pub(crate) struct StreamIdentity {
    pub(crate) api: String,
    pub(crate) provider: String,
    pub(crate) model: String,
}

impl StreamIdentity {
    pub(crate) fn new(
        api: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api: api.into(),
            provider: provider.into(),
            model: model.into(),
        }
    }
}

pub(crate) fn spawn_stream_worker(
    stream: AssistantMessageEventStream,
    identity: StreamIdentity,
    task: impl Future<Output = ()> + Send + 'static,
) {
    spawn_supervised_worker(task, move |message| {
        stream.fail(&identity.api, &identity.provider, &identity.model, message);
    });
}

pub(crate) fn spawn_supervised_worker(
    task: impl Future<Output = ()> + Send + 'static,
    fail: impl Fn(String) + Send + Sync + 'static,
) {
    let fail = Arc::new(fail);
    let supervised_fail = Arc::clone(&fail);
    let supervised = async move {
        match AssertUnwindSafe(task).catch_unwind().await {
            Ok(()) => supervised_fail("stream worker exited without a terminal event".to_owned()),
            Err(panic) => {
                supervised_fail(format!("stream worker panicked: {}", panic_message(&panic)))
            }
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(supervised);
    } else {
        std::thread::spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(supervised),
                Err(error) => fail(format!("stream runtime construction failed: {error}")),
            }
        });
    }
}

pub(crate) fn spawn_blocking_stream_worker(
    stream: AssistantMessageEventStream,
    identity: StreamIdentity,
    task: impl FnOnce() + Send + 'static,
) {
    std::thread::spawn(move || match catch_unwind(AssertUnwindSafe(task)) {
        Ok(()) => stream.fail(
            &identity.api,
            &identity.provider,
            &identity.model,
            "stream worker exited without a terminal event".to_owned(),
        ),
        Err(panic) => stream.fail(
            &identity.api,
            &identity.provider,
            &identity.model,
            format!("stream worker panicked: {}", panic_message(&panic)),
        ),
    });
}

fn panic_message(panic: &Box<dyn Any + Send>) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageRole,
        DoneStopReason, StopReason, TextContent, TextContentType, Usage,
    };
    use std::time::Duration;

    fn message(text: &str) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: text.to_owned(),
                text_signature: None,
            })],
            api: "test-api".to_owned(),
            provider: "test-provider".to_owned(),
            model: "test-model".to_owned(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }

    fn spawn(task: impl Future<Output = ()> + Send + 'static) -> AssistantMessageEventStream {
        let stream = AssistantMessageEventStream::new();
        spawn_stream_worker(
            stream.clone(),
            StreamIdentity::new("test-api", "test-provider", "test-model"),
            task,
        );
        stream
    }

    #[tokio::test(flavor = "current_thread")]
    async fn supervises_panics_and_missing_terminals() {
        for stream in [spawn(async { panic!("before start") }), spawn(async {})] {
            let result = tokio::time::timeout(Duration::from_secs(1), stream.result())
                .await
                .expect("supervisor must terminalize");
            assert_eq!(result.stop_reason, StopReason::Error);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn panic_after_delta_preserves_last_partial() {
        let stream = AssistantMessageEventStream::new();
        let producer = stream.clone();
        spawn_stream_worker(
            stream.clone(),
            StreamIdentity::new("test-api", "test-provider", "test-model"),
            async move {
                producer.push(AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "partial".to_owned(),
                    partial: message("partial").into(),
                });
                panic!("after delta");
            },
        );
        let result = tokio::time::timeout(Duration::from_secs(1), stream.result())
            .await
            .expect("panic must terminalize");
        assert_eq!(result.stop_reason, StopReason::Error);
        assert!(matches!(
            result.content.first(),
            Some(AssistantContentBlock::Text(text)) if text.text == "partial"
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn terminal_then_panic_keeps_the_first_terminal() {
        let stream = AssistantMessageEventStream::new();
        let producer = stream.clone();
        spawn_stream_worker(
            stream.clone(),
            StreamIdentity::new("test-api", "test-provider", "test-model"),
            async move {
                producer.push(AssistantMessageEvent::Done {
                    reason: DoneStopReason::Stop,
                    message: message("done"),
                });
                panic!("after done");
            },
        );
        let result = tokio::time::timeout(Duration::from_secs(1), stream.result())
            .await
            .expect("done must resolve");
        assert_eq!(result.stop_reason, StopReason::Stop);
    }
}
