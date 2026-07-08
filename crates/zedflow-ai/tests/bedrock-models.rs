//! Port of Pi `packages/ai/test/bedrock-models.test.ts`.

use zedflow_ai::api::lazy::{AssistantContent, AssistantMessage, Context, Model};
use zedflow_ai::compat::complete;
use zedflow_ai::providers::amazon_bedrock_models::amazon_bedrock_models;

const LIVE_BLOCKER: &str = "live Amazon Bedrock provider calls skipped; compat builtin dispatch and Bedrock transport are still PORT PLACEHOLDERs";

fn non_empty_env(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn has_bedrock_credentials() -> bool {
    non_empty_env("AWS_PROFILE")
        || (non_empty_env("AWS_ACCESS_KEY_ID") && non_empty_env("AWS_SECRET_ACCESS_KEY"))
        || non_empty_env("AWS_BEARER_TOKEN_BEDROCK")
}

fn compat_model(model: &zedflow_ai::models::Model) -> Model {
    Model::new(&model.id, &model.api, &model.provider)
}

fn text_content(response: &AssistantMessage) -> String {
    response
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Opaque(text) => text.as_str(),
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[test]
fn gets_all_available_bedrock_models() {
    let models = amazon_bedrock_models();

    assert!(!models.is_empty());
    println!("Found {} Bedrock models", models.len());
}

#[test]
#[ignore = "live Bedrock provider parity test skipped; see LIVE_BLOCKER"]
fn makes_a_simple_request_with_each_bedrock_model_live_parity() {
    if !(has_bedrock_credentials() && non_empty_env("BEDROCK_EXTENSIVE_MODEL_TEST")) {
        return;
    }

    for model in amazon_bedrock_models() {
        let model = compat_model(&model);
        let context = Context;
        let response = complete(&model, &context, None)
            .unwrap_or_else(|error| panic!("{LIVE_BLOCKER}: {error}"));

        assert_eq!(response.role, "assistant");
        assert!(!response.content.is_empty());
        assert!(response.usage.input + response.usage.cache_read > 0);
        assert!(response.usage.output > 0);
        assert!(response.error_message.is_none());

        let text_content = text_content(&response);
        assert!(!text_content.is_empty());
        let preview = text_content.chars().take(100).collect::<String>();
        println!("{}: {preview}", model.id);
    }
}
