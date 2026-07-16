//! Public facade checks for the Pi-compatible root barrel.
//!
//! Node dynamic-import specifier observability is JS-only; Rust keeps this facade
//! side-effect-free and leaves dependency-specific/live transports behind module paths.

use std::error::Error;

use futures::StreamExt;
use futures::executor::block_on;

use zedflow_ai::{
    AnthropicEffort, AssistantMessageEvent, AssistantMessageEventStream, Context,
    GoogleThinkingLevel, InMemoryCredentialStore, Model, Models, OAuthProviderId,
    OpenAIResponsesOptions, ProviderHookError, ProviderStreams, SimpleStreamOptions, StreamOptions,
    create_assistant_message_event_stream, default_provider_auth_context,
};

#[test]
fn root_facade_exports_core_pi_types_without_provider_side_effects() {
    let _: Models = Models::default();
    let _: Model = Model::default();
    let _: Context = Context::default();
    let _: StreamOptions = StreamOptions::default();
    let _: AssistantMessageEventStream = create_assistant_message_event_stream();
    let _: InMemoryCredentialStore = InMemoryCredentialStore::new();
    let _ = default_provider_auth_context();
    let _: OAuthProviderId = "openai-codex".to_owned();

    assert_eq!(zedflow_ai::INDEX_ENTRYPOINT, "@earendil-works/pi-ai");
    assert_eq!(AnthropicEffort::Medium, AnthropicEffort::Medium);
    assert_eq!(GoogleThinkingLevel::High, GoogleThinkingLevel::High);
    assert!(OpenAIResponsesOptions::default().temperature.is_none());
}

#[test]
fn rust_facade_documents_js_only_dynamic_import_observability() {
    // Pi can observe exact Node import specifiers with registerHooks. Rust cannot.
    // The equivalent invariant for the root facade is that constructing common core
    // values above does not initialize provider transports or require live credentials.
    assert_eq!(zedflow_ai::CRATE_NAME, "zedflow-ai");
}

#[test]
fn provider_hook_error_preserves_its_source() {
    let error = ProviderHookError::new(std::io::Error::other("hook rejected"));

    assert_eq!(error.to_string(), "hook rejected");
    assert!(error.source().is_some());
}

#[test]
fn lazy_chat_entrypoints_have_canonical_identity_and_terminal_error_contract() {
    let entries: Vec<(&str, ProviderStreams)> = vec![
        (
            "anthropic-messages",
            zedflow_ai::api::anthropic_messages_lazy::anthropic_messages_api(),
        ),
        (
            "azure-openai-responses",
            zedflow_ai::api::azure_openai_responses_lazy::azure_open_ai_responses_api(),
        ),
        (
            "bedrock-converse-stream",
            zedflow_ai::api::bedrock_converse_stream_lazy::bedrock_converse_stream_api(),
        ),
        (
            "google-generative-ai",
            zedflow_ai::api::google_generative_ai_lazy::google_generative_ai_api(),
        ),
        (
            "google-vertex",
            zedflow_ai::api::google_vertex_lazy::google_vertex_api(),
        ),
        (
            "mistral-conversations",
            zedflow_ai::api::mistral_conversations_lazy::mistral_conversations_api(),
        ),
        (
            "openai-codex-responses",
            zedflow_ai::api::openai_codex_responses_lazy::open_ai_codex_responses_api(),
        ),
        (
            "openai-completions",
            zedflow_ai::api::openai_completions_lazy::open_ai_completions_api(),
        ),
        (
            "openai-responses",
            zedflow_ai::api::openai_responses_lazy::open_ai_responses_api(),
        ),
    ];
    let context = Context::default();
    let options = StreamOptions {
        temperature: Some(0.25),
        ..StreamOptions::default()
    };
    let simple = SimpleStreamOptions {
        stream: options.clone(),
        ..SimpleStreamOptions::default()
    };

    for (api, streams) in entries {
        let model = Model {
            id: "model".into(),
            api: api.into(),
            provider: "provider".into(),
            ..Model::default()
        };
        let mut stream: AssistantMessageEventStream =
            (streams.stream)(&model, &context, Some(&options));
        let event = block_on(stream.next()).expect("terminal error");
        let AssistantMessageEvent::Error { error, .. } = event else {
            panic!("expected error");
        };
        assert_eq!(error.api, api);
        assert_eq!(error.provider, model.provider);
        assert!(
            error
                .error_message
                .as_deref()
                .is_some_and(|message| !message.is_empty())
        );
        assert!(
            !error
                .error_message
                .as_deref()
                .unwrap_or_default()
                .contains("synchronous legacy")
        );
        assert_eq!(block_on(stream.next()), None);
        assert_eq!(block_on(stream.result()), error);

        let _: AssistantMessageEventStream =
            (streams.stream_simple)(&model, &context, Some(&simple));
    }

    let model = Model {
        id: "compat-model".into(),
        api: "anthropic-messages".into(),
        provider: "custom-provider".into(),
        ..Model::default()
    };
    let _: AssistantMessageEventStream =
        zedflow_ai::compat::stream(&model, &context, Some(options)).expect("compat stream");
}
