use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::StreamExt;
use futures::executor::block_on;
use serde_json::json;
use zedflow_ai::ProviderHookError;
use zedflow_ai::api::openai_completions::{
    Context as OpenAIContext, Model as OpenAIModel, OpenAICompletionsOptions, stream_live,
};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, DoneStopReason, ErrorStopReason, StopReason, TextContent,
    TextContentType, ToolCall, ToolCallType, Usage, UsageCost,
};

fn message(stop_reason: StopReason, text: &str) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: if text.is_empty() {
            Vec::new()
        } else {
            vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: text.to_owned(),
                text_signature: None,
            })]
        },
        api: "test-api".to_owned(),
        provider: "test-provider".to_owned(),
        model: "test-model".to_owned(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input: 1,
            output: text.len() as u64,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: 1 + text.len() as u64,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason,
        error_message: None,
        timestamp: 0,
    }
}

#[test]
fn assistant_event_json_names_match_pi() {
    let partial = message(StopReason::Stop, "");
    let tool_call = ToolCall {
        content_type: ToolCallType::ToolCall,
        id: "call-1".to_owned(),
        name: "math".to_owned(),
        arguments: json!({ "a": 1 })
            .as_object()
            .expect("object literal")
            .clone()
            .into_iter()
            .collect(),
        thought_signature: None,
    };

    let text_start = serde_json::to_value(AssistantMessageEvent::TextStart {
        content_index: 2,
        partial: partial.clone().into(),
    })
    .expect("text_start event serializes");
    assert_eq!(text_start["type"], "text_start");
    assert_eq!(text_start["contentIndex"], 2);
    assert!(text_start.get("content_index").is_none());

    let tool_end = serde_json::to_value(AssistantMessageEvent::ToolcallEnd {
        content_index: 3,
        tool_call,
        partial: partial.into(),
    })
    .expect("toolcall_end event serializes");
    assert_eq!(tool_end["type"], "toolcall_end");
    assert_eq!(tool_end["toolCall"]["type"], "toolCall");
    assert!(tool_end.get("tool_call").is_none());

    let done = serde_json::to_value(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message: message(StopReason::Stop, "done"),
    })
    .expect("done event serializes");
    assert_eq!(done["type"], "done");

    let error = serde_json::to_value(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Error,
        error: message(StopReason::Error, "partial"),
    })
    .expect("error event serializes");
    assert_eq!(error["type"], "error");
}

#[test]
fn assistant_stream_preserves_iteration_order_and_done_result() {
    let stream = AssistantMessageEventStream::new();
    let start = message(StopReason::Stop, "");
    let partial = message(StopReason::Stop, "hel");
    let final_message = message(StopReason::Stop, "hello");

    stream.push(AssistantMessageEvent::Start {
        partial: start.clone().into(),
    });
    stream.push(AssistantMessageEvent::TextStart {
        content_index: 0,
        partial: start.into(),
    });
    stream.push(AssistantMessageEvent::TextDelta {
        content_index: 0,
        delta: "hel".to_owned(),
        partial: partial.into(),
    });
    stream.push(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message: final_message.clone(),
    });

    assert_eq!(block_on(stream.result()), final_message);

    let events = block_on(stream.collect::<Vec<_>>());
    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));
    assert!(matches!(events[1], AssistantMessageEvent::TextStart { .. }));
    assert!(matches!(events[2], AssistantMessageEvent::TextDelta { .. }));
    assert!(matches!(events[3], AssistantMessageEvent::Done { .. }));
    assert_eq!(events.len(), 4);
    let partials = events[..3]
        .iter()
        .filter_map(|event| match event {
            AssistantMessageEvent::Start { partial }
            | AssistantMessageEvent::TextStart { partial, .. }
            | AssistantMessageEvent::TextDelta { partial, .. } => Some(partial),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(partials.windows(2).all(|pair| pair[0].ptr_eq(pair[1])));
    assert!(
        partials
            .iter()
            .all(|partial| partial.snapshot() == final_message)
    );
}

#[test]
fn assistant_stream_error_result_returns_terminal_error_message() {
    let stream = AssistantMessageEventStream::new();
    let mut error_message = message(StopReason::Error, "bad");
    error_message.error_message = Some("provider failed".to_owned());

    stream.push(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Error,
        error: error_message.clone(),
    });
    stream.push(AssistantMessageEvent::TextDelta {
        content_index: 0,
        delta: "ignored".to_owned(),
        partial: message(StopReason::Stop, "ignored").into(),
    });

    assert!(stream.is_done());
    assert_eq!(block_on(stream.result()), error_message);
    assert_eq!(block_on(stream.collect::<Vec<_>>()).len(), 1);
}

#[test]
fn assistant_stream_end_with_result_emits_one_matching_terminal_event() {
    let stream = AssistantMessageEventStream::new();
    let done = message(StopReason::Stop, "done");
    stream.end(Some(done.clone()));

    assert_eq!(block_on(stream.result()), done);
    let events = block_on(stream.collect::<Vec<_>>());
    assert!(matches!(
        events.as_slice(),
        [AssistantMessageEvent::Done { reason: DoneStopReason::Stop, message }] if message == &done
    ));

    let stream = AssistantMessageEventStream::new();
    let aborted = message(StopReason::Aborted, "partial");
    stream.end(Some(aborted.clone()));

    assert_eq!(block_on(stream.result()), aborted);
    let events = block_on(stream.collect::<Vec<_>>());
    assert!(matches!(
        events.as_slice(),
        [AssistantMessageEvent::Error { reason: ErrorStopReason::Aborted, error }] if error == &aborted
    ));
}

#[test]
fn assistant_stream_aborted_error_result_preserves_partial_message() {
    let stream = AssistantMessageEventStream::new();
    let partial = message(StopReason::Aborted, "partial output");
    let mut aborted = partial.clone();
    aborted.error_message = Some("aborted".to_owned());

    stream.push(AssistantMessageEvent::TextStart {
        content_index: 0,
        partial: message(StopReason::Stop, "").into(),
    });
    stream.push(AssistantMessageEvent::TextDelta {
        content_index: 0,
        delta: "partial output".to_owned(),
        partial: partial.into(),
    });
    stream.push(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Aborted,
        error: aborted.clone(),
    });

    let result = block_on(stream.result());
    assert_eq!(result.stop_reason, StopReason::Aborted);
    assert_eq!(result.content, aborted.content);
    assert_eq!(result.error_message.as_deref(), Some("aborted"));

    let events = block_on(stream.collect::<Vec<_>>());
    assert!(matches!(events[0], AssistantMessageEvent::TextStart { .. }));
    assert!(matches!(events[1], AssistantMessageEvent::TextDelta { .. }));
    assert!(matches!(events[2], AssistantMessageEvent::Error { .. }));
}

#[test]
fn rejected_payload_hook_emits_one_terminal_error_without_later_events() {
    let calls = Arc::new(AtomicUsize::new(0));
    let hook_calls = Arc::clone(&calls);
    let options = OpenAICompletionsOptions {
        api_key: Some("test-key".to_owned()),
        on_payload: Some(Arc::new(move |_payload, _model| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(ProviderHookError::new(std::io::Error::other(
                    "payload rejected",
                )))
            })
        })),
        ..OpenAICompletionsOptions::default()
    };
    let model = OpenAIModel {
        id: "test-model".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "test-provider".to_owned(),
        base_url: "https://example.invalid".to_owned(),
        input: Vec::new(),
        reasoning: false,
        thinking_level_map: Default::default(),
        headers: Default::default(),
        max_tokens: 1,
        context_window: None,
        compat: None,
    };

    let events = block_on(
        stream_live(&model, &OpenAIContext::default(), Some(&options))
            .expect("stream creation succeeds")
            .collect::<Vec<_>>(),
    );

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(events.len(), 1);
    let [AssistantMessageEvent::Error { error, .. }] = events.as_slice() else {
        panic!("expected exactly one terminal error event");
    };
    assert_eq!(error.error_message.as_deref(), Some("payload rejected"));
}
