//! Deterministic port of Pi `packages/ai/test/empty.test.ts`.

use std::sync::Arc;

use futures::executor::block_on;
use zedflow_ai::compat;
use zedflow_ai::providers::faux::{
    FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message,
};
use zedflow_ai::types::{
    AssistantMessage, Context, Message, UserMessage, UserMessageContent, UserMessageRole,
};

fn user(content: UserMessageContent) -> Message {
    Message::User(UserMessage {
        role: UserMessageRole::User,
        content,
        timestamp: 1,
    })
}

#[test]
fn compat_complete_handles_empty_messages_through_faux_transport() {
    let registration = compat::register_faux_provider(RegisterFauxProviderOptions::default());
    let model = registration.get_model(None).expect("faux model");

    let mut empty_assistant = faux_assistant_message("");
    empty_assistant.content.clear();
    let contexts = vec![
        Context {
            messages: vec![user(UserMessageContent::Blocks(vec![]))],
            ..Context::default()
        },
        Context {
            messages: vec![user(UserMessageContent::Text(String::new()))],
            ..Context::default()
        },
        Context {
            messages: vec![user(UserMessageContent::Text("   \n\t  ".into()))],
            ..Context::default()
        },
        Context {
            messages: vec![
                user(UserMessageContent::Text("Hello, how are you?".into())),
                Message::Assistant(empty_assistant),
                user(UserMessageContent::Text("Please respond this time.".into())),
            ],
            ..Context::default()
        },
    ];

    registration.set_responses(
        (0..contexts.len())
            .map(|_| {
                FauxResponseStep::Factory(Arc::new(|context, _, _, _| {
                    assert!(!context.messages.is_empty());
                    faux_assistant_message("handled")
                }))
            })
            .collect(),
    );

    for context in contexts {
        let response: AssistantMessage =
            block_on(compat::complete(&model, &context, None)).expect("compat complete");
        assert_eq!(
            response.role,
            zedflow_ai::types::AssistantMessageRole::Assistant
        );
        assert!(!response.content.is_empty());
        assert!(response.error_message.is_none());
    }

    assert_eq!(registration.state.call_count(), 4);
    registration.unregister();
}
