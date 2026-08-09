//! Deterministic provider-dispatch coverage for Pi `packages/ai/test/stream.test.ts`.

use futures::executor::block_on;
use zedflow_ai::compat;
use zedflow_ai::providers::faux::{
    FauxModelDefinition, FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message,
};
use zedflow_ai::types::{AssistantContentBlock, Context};

const BUILTIN_API_MATRIX: &[&str] = &[
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

#[test]
fn compat_complete_dispatches_every_builtin_api_to_registered_faux_transport() {
    for api in BUILTIN_API_MATRIX {
        let registration = compat::register_faux_provider(RegisterFauxProviderOptions {
            api: Some((*api).into()),
            provider: Some(format!("oracle-{api}")),
            models: vec![FauxModelDefinition {
                id: "oracle-model".into(),
                ..FauxModelDefinition::default()
            }],
            ..RegisterFauxProviderOptions::default()
        });
        registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
            *api,
        ))]);

        let model = registration.get_model(None).expect("faux model");
        let response =
            block_on(compat::complete(&model, &Context::default(), None)).expect("compat complete");
        assert_eq!(response.api, *api);
        assert_eq!(response.provider, format!("oracle-{api}"));
        assert_eq!(response.model, "oracle-model");
        assert!(matches!(
            response.content.as_slice(),
            [AssistantContentBlock::Text(text)] if text.text == *api
        ));

        registration.unregister();
    }

    compat::reset_api_providers().expect("restore builtin providers");
}
