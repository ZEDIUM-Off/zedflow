//! Differential check against the dependency-free frozen TypeScript compat oracle.

use std::process::Command;
use std::sync::Arc;

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::compat;
use zedflow_ai::providers::faux::{
    FauxModelDefinition, FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message,
};
use zedflow_ai::types::{
    AssistantContentBlock, Context, Message, UserMessage, UserMessageContent, UserMessageRole,
};

const APIS: &[&str] = &[
    "anthropic-messages",
    "openai-completions",
    "openai-responses",
    "openai-codex-responses",
    "azure-openai-responses",
    "google-generative-ai",
    "google-vertex",
    "mistral-conversations",
    "bedrock-converse-stream",
];

fn user(content: UserMessageContent) -> Message {
    Message::User(UserMessage {
        role: UserMessageRole::User,
        content,
        timestamp: 1,
    })
}

fn describe_context(context: &Context) -> String {
    context
        .messages
        .iter()
        .map(|message| match message {
            Message::User(message) => match &message.content {
                UserMessageContent::Text(text) => format!("user:text:{}", text.chars().count()),
                UserMessageContent::Blocks(blocks) => format!("user:blocks:{}", blocks.len()),
            },
            Message::Assistant(message) => format!("assistant:blocks:{}", message.content.len()),
            Message::ToolResult(message) => format!("toolResult:blocks:{}", message.content.len()),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn rust_observation() -> Value {
    let empty_registration = compat::register_faux_provider(RegisterFauxProviderOptions {
        api: Some("oracle-empty".into()),
        ..RegisterFauxProviderOptions::default()
    });
    empty_registration.set_responses(
        (0..4)
            .map(|_| {
                FauxResponseStep::Factory(Arc::new(|context, _, _, _| {
                    faux_assistant_message(describe_context(context))
                }))
            })
            .collect(),
    );
    let empty_model = empty_registration.get_model(None).expect("faux model");
    let mut empty_assistant = faux_assistant_message("");
    empty_assistant.content.clear();
    let empty_contexts = [
        (
            "content-array",
            Context {
                messages: vec![user(UserMessageContent::Blocks(vec![]))],
                ..Context::default()
            },
        ),
        (
            "empty-string",
            Context {
                messages: vec![user(UserMessageContent::Text(String::new()))],
                ..Context::default()
            },
        ),
        (
            "whitespace",
            Context {
                messages: vec![user(UserMessageContent::Text("   \n\t  ".into()))],
                ..Context::default()
            },
        ),
        (
            "empty-assistant",
            Context {
                messages: vec![
                    user(UserMessageContent::Text("hello".into())),
                    Message::Assistant(empty_assistant),
                    user(UserMessageContent::Text("respond".into())),
                ],
                ..Context::default()
            },
        ),
    ];
    let empty: Vec<_> = empty_contexts
        .iter()
        .map(|(input, context)| {
            let response = block_on(compat::complete(&empty_model, context, None))
                .expect("empty-message compat complete");
            let text = match response.content.as_slice() {
                [AssistantContentBlock::Text(text)] => text.text.clone(),
                blocks => panic!("unexpected faux content: {blocks:?}"),
            };
            json!({
                "input": input,
                "role": "assistant",
                "contentDefined": true,
                "error": response.error_message.is_some(),
                "text": text,
            })
        })
        .collect();
    empty_registration.unregister();

    let dispatch: Vec<_> = APIS
        .iter()
        .map(|api| {
            let registration = compat::register_faux_provider(RegisterFauxProviderOptions {
                api: Some((*api).into()),
                provider: Some(format!("oracle-{api}")),
                models: vec![FauxModelDefinition {
                    id: "oracle-model".into(),
                    ..FauxModelDefinition::default()
                }],
                ..RegisterFauxProviderOptions::default()
            });
            registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(
                |context, _, _, model| {
                    faux_assistant_message(format!("{}:{}", model.api, describe_context(context)))
                },
            ))]);
            let context = Context {
                messages: vec![user(UserMessageContent::Text((*api).into()))],
                ..Context::default()
            };
            let response = block_on(compat::complete(
                &registration.get_model(None).expect("faux model"),
                &context,
                None,
            ))
            .expect("dispatch compat complete");
            let text = match response.content.as_slice() {
                [AssistantContentBlock::Text(text)] => text.text.clone(),
                blocks => panic!("unexpected faux content: {blocks:?}"),
            };
            registration.unregister();
            json!({
                "api": response.api,
                "provider": response.provider,
                "model": response.model,
                "role": "assistant",
                "text": text,
            })
        })
        .collect();
    compat::reset_api_providers().expect("restore builtin providers");

    json!({ "empty": empty, "dispatch": dispatch })
}

#[test]
fn rust_compat_matches_frozen_typescript_oracle() {
    let fixture = format!(
        "{}/tests/fixtures/frozen-oracle.ts",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = Command::new("node")
        .arg(fixture)
        .output()
        .expect("Node 22+ is required to run the frozen TypeScript oracle");
    assert!(
        output.status.success(),
        "TypeScript oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let oracle: Value = serde_json::from_slice(&output.stdout).expect("oracle JSON");

    assert_eq!(rust_observation(), oracle);
}
