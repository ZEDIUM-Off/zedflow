//! Deterministic port of Pi `packages/ai/test/abort.test.ts` through the faux provider.

use futures::{StreamExt, executor::block_on};
use zedflow_ai::providers::faux::{
    FauxResponseStep, FauxTokenSize, RegisterFauxProviderOptions, faux_assistant_message,
    faux_provider,
};
use zedflow_ai::types;

fn event_name(event: &types::AssistantMessageEvent) -> &'static str {
    match event {
        types::AssistantMessageEvent::Start { .. } => "start",
        types::AssistantMessageEvent::TextStart { .. } => "text_start",
        types::AssistantMessageEvent::TextDelta { .. } => "text_delta",
        types::AssistantMessageEvent::TextEnd { .. } => "text_end",
        types::AssistantMessageEvent::ThinkingStart { .. } => "thinking_start",
        types::AssistantMessageEvent::ThinkingDelta { .. } => "thinking_delta",
        types::AssistantMessageEvent::ThinkingEnd { .. } => "thinking_end",
        types::AssistantMessageEvent::ToolcallStart { .. } => "toolcall_start",
        types::AssistantMessageEvent::ToolcallDelta { .. } => "toolcall_delta",
        types::AssistantMessageEvent::ToolcallEnd { .. } => "toolcall_end",
        types::AssistantMessageEvent::Done { .. } => "done",
        types::AssistantMessageEvent::Error { .. } => "error",
    }
}

fn text(message: &types::AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            types::AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

fn paced_faux() -> zedflow_ai::providers::faux::FauxProviderHandle {
    faux_provider(RegisterFauxProviderOptions {
        tokens_per_second: Some(100.0),
        token_size: FauxTokenSize {
            min: Some(1),
            max: Some(1),
        },
        ..RegisterFauxProviderOptions::default()
    })
}

#[test]
fn abort_mid_stream_is_terminal_and_next_request_still_runs() {
    let faux = paced_faux();
    let model = faux.provider.get_models().first().cloned().expect("model");
    faux.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("abcdefghijklmnopqrstuvwxyz")),
        FauxResponseStep::Message(faux_assistant_message("follow up")),
    ]);
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    let options = types::StreamOptions {
        signal: Some(controller.signal()),
        ..types::StreamOptions::default()
    };
    let mut stream = faux
        .provider
        .stream(&model, &types::Context::default(), Some(&options));

    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            if matches!(event, types::AssistantMessageEvent::TextDelta { .. }) {
                controller.abort();
            }
            events.push(event);
        }
        events
    });

    assert_eq!(
        events.iter().map(event_name).collect::<Vec<_>>(),
        ["start", "text_start", "text_delta", "error"]
    );
    let aborted = match events.last().expect("terminal abort") {
        types::AssistantMessageEvent::Error { reason, error } => {
            assert_eq!(*reason, types::ErrorStopReason::Aborted);
            error
        }
        event => panic!("expected terminal abort, got {event:?}"),
    };
    assert_eq!(aborted.stop_reason, types::StopReason::Aborted);
    assert_eq!(text(aborted), "abcd");

    let follow_up = block_on(
        faux.provider
            .stream(&model, &types::Context::default(), None)
            .result(),
    );
    assert_eq!(follow_up.stop_reason, types::StopReason::Stop);
    assert_eq!(text(&follow_up), "follow up");
}

#[test]
fn immediate_abort_emits_exactly_one_terminal_event() {
    let faux = paced_faux();
    let model = faux.provider.get_models().first().cloned().expect("model");
    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "discarded",
    ))]);
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    controller.abort();
    let options = types::StreamOptions {
        signal: Some(controller.signal()),
        ..types::StreamOptions::default()
    };
    let stream = faux
        .provider
        .stream(&model, &types::Context::default(), Some(&options));
    let result_stream = stream.clone();

    let events = block_on(stream.collect::<Vec<_>>());
    let response = block_on(result_stream.result());

    assert_eq!(events.iter().map(event_name).collect::<Vec<_>>(), ["error"]);
    assert_eq!(response.stop_reason, types::StopReason::Aborted);
    assert!(response.content.is_empty());
    assert_eq!(
        response.error_message.as_deref(),
        Some("Request was aborted")
    );
}

#[test]
fn abort_then_new_message_uses_a_fresh_signal() {
    let faux = paced_faux();
    let model = faux.provider.get_models().first().cloned().expect("model");
    faux.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("discarded")),
        FauxResponseStep::Message(faux_assistant_message("four")),
    ]);
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    controller.abort();
    let options = types::StreamOptions {
        signal: Some(controller.signal()),
        ..types::StreamOptions::default()
    };

    let aborted = block_on(
        faux.provider
            .stream(&model, &types::Context::default(), Some(&options))
            .result(),
    );
    assert_eq!(aborted.stop_reason, types::StopReason::Aborted);
    assert!(aborted.content.is_empty());

    let context = types::Context {
        messages: vec![
            types::Message::Assistant(aborted),
            types::Message::User(types::UserMessage {
                role: types::UserMessageRole::User,
                content: types::UserMessageContent::Text("What is 2 + 2?".to_owned()),
                timestamp: 1,
            }),
        ],
        ..types::Context::default()
    };
    let follow_up = block_on(faux.provider.stream(&model, &context, None).result());

    assert_eq!(follow_up.stop_reason, types::StopReason::Stop);
    assert_eq!(text(&follow_up), "four");
    assert_eq!(faux.state.call_count(), 2);
}
