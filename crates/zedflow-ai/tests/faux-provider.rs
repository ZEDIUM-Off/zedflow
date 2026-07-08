//! Port of Pi `packages/ai/test/faux-provider.test.ts`.
//!
//! The Rust faux source is still partial: `compat::register_faux_provider` is a
//! PORT PLACEHOLDER, `Context`/content/event types are opaque placeholders, and
//! token/cache/abort behavior is not ported. Runnable tests below cover the local
//! queue/model behavior that exists; exact compat parity cases stay ignored.

use std::sync::Arc;

use serde_json::json;
use zedflow_ai::api::lazy::{AssistantContent, Context, StopReason};
use zedflow_ai::providers::faux::{
    FauxModelDefinition, FauxResponseStep, RegisterFauxProviderOptions, create_faux_core,
    faux_assistant_message,
};

const BLOCKER: &str = "PORT PLACEHOLDER: compat::register_faux_provider, typed Context/content events, usage/cache accounting, async factories, and abort pacing are not fully ported yet";

fn opaque_content(message: &zedflow_ai::api::lazy::AssistantMessage) -> Vec<String> {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Opaque(text) => text.clone(),
        })
        .collect()
}

fn next_message(
    faux: &zedflow_ai::providers::faux::FauxCore,
    model: &zedflow_ai::api::lazy::Model,
) -> zedflow_ai::api::lazy::AssistantMessage {
    faux.streams
        .stream(model, &Context, None)
        .result()
        .cloned()
        .expect("faux stream should expose a final message")
}

fn blocked(case: &str) {
    panic!("{BLOCKER}; source case: {case}");
}

#[test]
fn consumes_queued_responses_in_order_and_errors_when_exhausted_for_ported_core() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("default faux model");
    faux.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("first")),
        FauxResponseStep::Message(faux_assistant_message("second")),
    ]);

    let first = next_message(&faux, &model);
    let second = next_message(&faux, &model);
    let exhausted = next_message(&faux, &model);

    assert_eq!(opaque_content(&first), ["first"]);
    assert_eq!(opaque_content(&second), ["second"]);
    assert_eq!(exhausted.stop_reason, StopReason::Error);
    assert_eq!(
        exhausted.error_message.as_deref(),
        Some("No more faux responses queued")
    );
    assert_eq!(faux.get_pending_response_count(), 0);
    assert_eq!(faux.state.call_count(), 3);
}

#[test]
fn replaces_and_appends_queued_responses_for_ported_core() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("default faux model");
    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "first",
    ))]);

    assert_eq!(opaque_content(&next_message(&faux, &model)), ["first"]);
    assert_eq!(faux.get_pending_response_count(), 0);

    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "second",
    ))]);
    assert_eq!(faux.get_pending_response_count(), 1);
    assert_eq!(opaque_content(&next_message(&faux, &model)), ["second"]);

    faux.append_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("third")),
        FauxResponseStep::Message(faux_assistant_message("fourth")),
    ]);
    assert_eq!(faux.get_pending_response_count(), 2);
    assert_eq!(opaque_content(&next_message(&faux, &model)), ["third"]);
    assert_eq!(opaque_content(&next_message(&faux, &model)), ["fourth"]);
    assert_eq!(faux.get_pending_response_count(), 0);
}

#[test]
fn rewrites_api_provider_and_model_on_returned_messages_for_ported_core() {
    let faux = create_faux_core(RegisterFauxProviderOptions {
        api: Some("faux:test".to_owned()),
        provider: Some("faux-provider".to_owned()),
        models: vec![FauxModelDefinition {
            id: "faux-model".to_owned(),
            ..FauxModelDefinition::default()
        }],
        ..RegisterFauxProviderOptions::default()
    });
    let model = faux.get_model(None).expect("configured faux model");
    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "hello",
    ))]);

    let response = next_message(&faux, &model);

    assert_eq!(response.api, "faux:test");
    assert_eq!(response.provider, "faux-provider");
    assert_eq!(response.model, "faux-model");
}

#[test]
fn supports_multiple_models_and_model_aware_factories_for_ported_core() {
    let faux = create_faux_core(RegisterFauxProviderOptions {
        models: vec![
            FauxModelDefinition {
                id: "faux-fast".to_owned(),
                name: Some("Faux Fast".to_owned()),
                reasoning: false,
                ..FauxModelDefinition::default()
            },
            FauxModelDefinition {
                id: "faux-thinker".to_owned(),
                name: Some("Faux Thinker".to_owned()),
                reasoning: true,
                ..FauxModelDefinition::default()
            },
        ],
        ..RegisterFauxProviderOptions::default()
    });
    faux.set_responses(vec![
        FauxResponseStep::Factory(Arc::new(|_, _, _, model| {
            faux_assistant_message(format!("{}", model.id))
        })),
        FauxResponseStep::Factory(Arc::new(|_, _, _, model| {
            faux_assistant_message(format!("{}", model.id))
        })),
    ]);

    assert_eq!(
        faux.models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["faux-fast", "faux-thinker"]
    );
    assert_eq!(faux.get_model(None), faux.models.first().cloned());

    let fast = next_message(
        &faux,
        &faux.get_model(Some("faux-fast")).expect("fast model"),
    );
    let thinker = next_message(
        &faux,
        &faux
            .get_model(Some("faux-thinker"))
            .expect("thinking model"),
    );

    assert_eq!(opaque_content(&fast), ["faux-fast"]);
    assert_eq!(opaque_content(&thinker), ["faux-thinker"]);
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat::register_faux_provider is not wired to providers::faux and usage estimates are not implemented"]
fn registers_a_custom_provider_and_estimates_usage() {
    blocked("registers a custom provider and estimates usage");
}

#[test]
#[ignore = "PORT PLACEHOLDER: AssistantContent is opaque; text/thinking/tool-call block variants are not ported"]
fn supports_helper_blocks_for_text_thinking_and_tool_calls() {
    assert_eq!(json!({ "text": "hi" }), json!({ "text": "hi" }));
    blocked("supports helper blocks for text, thinking, and tool calls");
}

#[test]
#[ignore = "PORT PLACEHOLDER: model reasoning metadata is not exposed on lazy::Model"]
fn supports_multiple_models_with_per_model_reasoning_and_model_aware_factories() {
    blocked("supports multiple models with per-model reasoning and model-aware factories");
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat::register_faux_provider is not wired to providers::faux"]
fn rewrites_api_provider_and_model_on_returned_messages() {
    blocked("rewrites api, provider, and model on returned messages");
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat::complete path cannot create a registered faux provider yet"]
fn consumes_queued_responses_in_order_and_errors_when_exhausted() {
    blocked("consumes queued responses in order and errors when exhausted");
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat::complete path cannot create a registered faux provider yet"]
fn can_replace_and_append_queued_responses() {
    blocked("can replace and append queued responses");
}

#[test]
#[ignore = "PORT PLACEHOLDER: FauxResponseFactory is synchronous; async response factories are not ported"]
fn supports_async_response_factories() {
    blocked("supports async response factories");
}

#[test]
#[ignore = "PORT PLACEHOLDER: FauxResponseFactory cannot return errors/panics as assistant error events yet"]
fn emits_an_error_when_a_response_factory_throws() {
    blocked("emits an error when a response factory throws");
}

#[test]
#[ignore = "PORT PLACEHOLDER: typed Context/messages/tools and usage estimation are not ported"]
fn estimates_prompt_and_output_tokens_from_serialized_context() {
    blocked("estimates prompt and output tokens from serialized context");
}

#[test]
#[ignore = "PORT PLACEHOLDER: sessionId/cacheRetention options and prompt-cache accounting are not ported"]
fn does_not_share_cache_across_sessions_or_requests_without_session_id() {
    blocked("does not share cache across sessions or requests without sessionId");
}

#[test]
#[ignore = "PORT PLACEHOLDER: sessionId/cacheRetention options and prompt-cache accounting are not ported"]
fn simulates_prompt_caching_per_session_id() {
    blocked("simulates prompt caching per sessionId");
}

#[test]
#[ignore = "PORT PLACEHOLDER: cacheRetention options and prompt-cache accounting are not ported"]
fn does_not_simulate_caching_when_cache_retention_is_none() {
    blocked("does not simulate caching when cacheRetention is none");
}

#[test]
#[ignore = "PORT PLACEHOLDER: typed stream delta events for thinking/text/tool calls are not ported"]
fn streams_thinking_text_and_partial_tool_call_deltas() {
    blocked("streams thinking, text, and partial tool call deltas");
}

#[test]
#[ignore = "PORT PLACEHOLDER: fixed-size chunking and exact stream event order are not ported"]
fn streams_an_exact_event_order_for_fixed_size_chunks() {
    blocked("streams an exact event order for fixed-size chunks");
}

#[test]
#[ignore = "PORT PLACEHOLDER: multiple typed tool-call stream events are not ported"]
fn streams_multiple_tool_calls_in_one_message() {
    blocked("streams multiple tool calls in one message");
}

#[test]
#[ignore = "PORT PLACEHOLDER: terminal error event ordering after partial deltas is not ported"]
fn streams_an_explicit_assistant_error_message_as_a_terminal_error() {
    blocked("streams an explicit assistant error message as a terminal error");
}

#[test]
#[ignore = "PORT PLACEHOLDER: terminal aborted event ordering after partial deltas is not ported"]
fn streams_an_explicit_assistant_aborted_message_as_a_terminal_error() {
    blocked("streams an explicit assistant aborted message as a terminal error");
}

#[test]
#[ignore = "PORT PLACEHOLDER: AbortSignal equivalent and pre-chunk abort handling are not ported"]
fn supports_aborting_before_the_first_chunk() {
    blocked("supports aborting before the first chunk");
}

#[test]
#[ignore = "PORT PLACEHOLDER: AbortSignal equivalent and paced text streaming are not ported"]
fn supports_aborting_mid_text_stream_when_paced() {
    blocked("supports aborting mid-text stream when paced");
}

#[test]
#[ignore = "PORT PLACEHOLDER: AbortSignal equivalent and paced thinking streaming are not ported"]
fn supports_aborting_mid_thinking_stream_when_paced() {
    blocked("supports aborting mid-thinking stream when paced");
}

#[test]
#[ignore = "PORT PLACEHOLDER: AbortSignal equivalent and paced tool-call streaming are not ported"]
fn supports_aborting_mid_toolcall_stream_when_paced() {
    blocked("supports aborting mid-toolcall stream when paced");
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat faux registration/unregistration is not ported"]
fn unregisters_the_provider() {
    blocked("unregisters the provider");
}
