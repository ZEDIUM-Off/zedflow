//! Port of Pi `packages/ai/test/azure-openai-base-url.test.ts`.

use std::collections::HashMap;

use zedflow_ai::api::azure_openai_responses::{self, AzureOpenAIResponsesOptions, Context, Model};

fn model() -> Model {
    Model {
        id: "gpt-4o-mini".to_string(),
        provider: "azure-openai-responses".to_string(),
        base_url: None,
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
    }
}

fn context() -> Context {
    Context
}

fn capture_client_base_url(base_url: &str) -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some(base_url.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .base_url
}

fn run_invalid_url() -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("not-a-url".to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect_err("invalid URL should fail")
        .to_string()
}

fn run_prompt_cache_key(session_id: &str) -> Option<String> {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("https://my-resource.openai.azure.com".to_string()),
        session_id: Some(session_id.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .body
        .get("prompt_cache_key")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn run_store_flag() -> Option<bool> {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("https://my-resource.openai.azure.com".to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .body
        .get("store")
        .and_then(serde_json::Value::as_bool)
}

fn capture_resource_name_base_url(resource_name: &str) -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_resource_name: Some(resource_name.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .base_url
}

#[test]
fn normalizes_cognitive_services_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://marc-quicktests-resource.cognitiveservices.azure.com"),
        "https://marc-quicktests-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_microsoft_foundry_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://marc-quicktests-resource.ai.azure.com"),
        "https://marc-quicktests-resource.ai.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_azure_openai_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.openai.azure.com"),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_openai_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.cognitiveservices.azure.com/openai"),
        "https://my-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn preserves_openai_v1_endpoints() {
    assert_eq!(
        capture_client_base_url("https://my-resource.cognitiveservices.azure.com/openai/v1"),
        "https://my-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_openai_v1_responses_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.services.ai.azure.com/openai/v1/responses"),
        "https://my-resource.services.ai.azure.com/openai/v1"
    );
}

#[test]
fn preserves_explicit_non_azure_proxy_paths() {
    assert_eq!(
        capture_client_base_url("https://my-proxy.example.com/v1"),
        "https://my-proxy.example.com/v1"
    );
}

#[test]
fn strips_query_params_when_normalizing_azure_host_urls() {
    assert_eq!(
        capture_client_base_url(
            "https://my-resource.openai.azure.com/openai?api-version=2024-12-01"
        ),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}

#[test]
fn preserves_query_params_on_non_azure_proxy_urls() {
    assert_eq!(
        capture_client_base_url("https://my-proxy.example.com/v1?custom=true"),
        "https://my-proxy.example.com/v1?custom=true"
    );
}

#[test]
fn throws_on_invalid_urls() {
    assert!(run_invalid_url().contains("invalid Azure OpenAI base URL"));
}

#[test]
fn clamps_prompt_cache_key_to_openais_64_character_limit() {
    assert_eq!(run_prompt_cache_key(&"x".repeat(67)), Some("x".repeat(64)));
}

#[test]
fn disables_server_side_response_storage() {
    assert_eq!(run_store_flag(), Some(false));
}

#[test]
fn builds_correct_default_url_from_azure_openai_resource_name() {
    assert_eq!(
        capture_resource_name_base_url("my-resource"),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}
