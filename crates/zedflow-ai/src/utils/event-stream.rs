//! Event stream helpers ported from Pi's `packages/ai/src/utils/event-stream.ts`.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

use futures::Stream;
use futures::future::poll_fn;

use crate::types::{
    AssistantMessage, AssistantMessageEvent, DoneStopReason, ErrorStopReason, StopReason,
};

/// In-memory async event stream with a separately awaited final result.
#[derive(Clone)]
pub struct EventStream<T, R = T> {
    inner: Arc<Mutex<EventStreamState<T, R>>>,
    is_complete: Arc<dyn Fn(&T) -> bool + Send + Sync>,
    extract_result: Arc<dyn Fn(&T) -> R + Send + Sync>,
}

struct EventStreamState<T, R> {
    queue: VecDeque<T>,
    stream_wakers: Vec<Waker>,
    result_wakers: Vec<Waker>,
    done: bool,
    final_result: Option<R>,
}

impl<T, R> EventStream<T, R> {
    /// Creates an event stream.
    #[must_use]
    pub fn new(
        is_complete: impl Fn(&T) -> bool + Send + Sync + 'static,
        extract_result: impl Fn(&T) -> R + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventStreamState {
                queue: VecDeque::new(),
                stream_wakers: Vec::new(),
                result_wakers: Vec::new(),
                done: false,
                final_result: None,
            })),
            is_complete: Arc::new(is_complete),
            extract_result: Arc::new(extract_result),
        }
    }

    /// Pushes an event, ignoring pushes after the stream has completed.
    pub fn push(&self, event: T) {
        let complete = (self.is_complete)(&event);
        let final_result = complete.then(|| (self.extract_result)(&event));
        let stream_wakers;
        let result_wakers;

        {
            let mut state = lock_state(&self.inner);
            if state.done {
                return;
            }

            if complete {
                state.done = true;
                if state.final_result.is_none() {
                    state.final_result = final_result;
                }
            }

            state.queue.push_back(event);
            stream_wakers = std::mem::take(&mut state.stream_wakers);
            result_wakers = if complete {
                std::mem::take(&mut state.result_wakers)
            } else {
                Vec::new()
            };
        }

        wake_all(stream_wakers);
        wake_all(result_wakers);
    }

    /// Ends the stream and optionally records the final result.
    pub fn end(&self, result: Option<R>) {
        let stream_wakers;
        let result_wakers;

        {
            let mut state = lock_state(&self.inner);
            state.done = true;

            let resolved = result.is_some() && state.final_result.is_none();
            if resolved {
                state.final_result = result;
            }

            stream_wakers = std::mem::take(&mut state.stream_wakers);
            result_wakers = if resolved {
                std::mem::take(&mut state.result_wakers)
            } else {
                Vec::new()
            };
        }

        wake_all(stream_wakers);
        wake_all(result_wakers);
    }

    /// Returns whether the stream is complete.
    #[must_use]
    pub fn is_done(&self) -> bool {
        lock_state(&self.inner).done
    }

    /// Waits for and returns the final result.
    ///
    /// Like Pi's unresolved promise, this future remains pending if the stream is ended without a
    /// result and no terminal event has produced one.
    pub async fn result(&self) -> R
    where
        R: Clone,
    {
        poll_fn(|context| {
            let mut state = lock_state(&self.inner);
            if let Some(result) = &state.final_result {
                return Poll::Ready(result.clone());
            }
            push_waker(&mut state.result_wakers, context.waker());
            Poll::Pending
        })
        .await
    }
}

impl<T, R> Stream for EventStream<T, R> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut state = lock_state(&self.inner);
        if let Some(event) = state.queue.pop_front() {
            return Poll::Ready(Some(event));
        }
        if state.done {
            return Poll::Ready(None);
        }
        push_waker(&mut state.stream_wakers, context.waker());
        Poll::Pending
    }
}

/// Event stream specialized for Pi assistant-message events.
#[derive(Clone)]
pub struct AssistantMessageEventStream {
    inner: EventStream<AssistantMessageEvent, AssistantMessage>,
}

impl AssistantMessageEventStream {
    /// Creates an empty assistant-message event stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: EventStream::new(is_terminal_assistant_event, |event| match event {
                AssistantMessageEvent::Done { message, .. } => message.clone(),
                AssistantMessageEvent::Error { error, .. } => error.clone(),
                _ => unreachable!("assistant terminal event matched by is_complete"),
            }),
        }
    }

    /// Pushes an assistant-message event, ignoring pushes after completion.
    pub fn push(&self, event: AssistantMessageEvent) {
        self.inner.push(event);
    }

    /// Ends the stream, translating a final result into its terminal event.
    pub fn end(&self, result: Option<AssistantMessage>) {
        let Some(message) = result else {
            self.inner.end(None);
            return;
        };

        match message.stop_reason {
            StopReason::Stop => self.push(AssistantMessageEvent::Done {
                reason: DoneStopReason::Stop,
                message,
            }),
            StopReason::Length => self.push(AssistantMessageEvent::Done {
                reason: DoneStopReason::Length,
                message,
            }),
            StopReason::ToolUse => self.push(AssistantMessageEvent::Done {
                reason: DoneStopReason::ToolUse,
                message,
            }),
            StopReason::Error => self.push(AssistantMessageEvent::Error {
                reason: ErrorStopReason::Error,
                error: message,
            }),
            StopReason::Aborted => self.push(AssistantMessageEvent::Error {
                reason: ErrorStopReason::Aborted,
                error: message,
            }),
        }
    }

    /// Returns whether the stream is complete.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.inner.is_done()
    }

    /// Waits for and returns the final assistant message.
    pub async fn result(&self) -> AssistantMessage {
        self.inner.result().await
    }
}

impl Default for AssistantMessageEventStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Stream for AssistantMessageEventStream {
    type Item = AssistantMessageEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(context)
    }
}

/// Creates an [`AssistantMessageEventStream`] for extension-style callers.
#[must_use]
pub fn create_assistant_message_event_stream() -> AssistantMessageEventStream {
    AssistantMessageEventStream::new()
}

fn is_terminal_assistant_event(event: &AssistantMessageEvent) -> bool {
    matches!(
        event,
        AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
    )
}

fn lock_state<T, R>(
    inner: &Mutex<EventStreamState<T, R>>,
) -> MutexGuard<'_, EventStreamState<T, R>> {
    inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn push_waker(wakers: &mut Vec<Waker>, waker: &Waker) {
    if !wakers.iter().any(|stored| stored.will_wake(waker)) {
        wakers.push(waker.clone());
    }
}

fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures::{FutureExt, StreamExt};

    #[test]
    fn queues_events_and_resolves_on_terminal_event() {
        let mut stream = EventStream::new(|event: &&str| *event == "done", |event: &&str| *event);

        stream.push("delta");
        stream.push("done");
        stream.push("ignored");

        assert!(stream.is_done());
        assert_eq!(block_on(stream.result()), "done");
        assert_eq!(block_on(stream.next()), Some("delta"));
        assert_eq!(block_on(stream.next()), Some("done"));
        assert_eq!(block_on(stream.next()), None);
    }

    #[test]
    fn waits_for_later_events() {
        let stream = EventStream::new(|event: &&str| *event == "done", |event: &&str| *event);
        let mut consumer = stream.clone();
        let next = consumer.next();

        assert!(next.now_or_never().is_none());
        stream.push("later");

        assert_eq!(block_on(consumer.next()), Some("later"));
    }

    #[test]
    fn end_with_result_resolves_without_queuing_event() {
        let mut stream = EventStream::new(|event: &&str| *event == "done", |event: &&str| *event);

        stream.push("delta");
        stream.end(Some("manual"));

        assert_eq!(block_on(stream.result()), "manual");
        assert_eq!(block_on(stream.next()), Some("delta"));
        assert_eq!(block_on(stream.next()), None);
    }
}
