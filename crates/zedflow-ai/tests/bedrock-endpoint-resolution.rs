//! Port of Pi `packages/ai/test/bedrock-endpoint-resolution.test.ts`.

use std::collections::HashMap;

use zedflow_ai::api::bedrock_converse_stream::{
    BedrockOptions, CacheRetention, Model, ProviderEnv, resolve_bedrock_client_config_with_env,
};

const EU_MODEL_ID: &str = "eu.anthropic.claude-sonnet-4-5-20250929-v1:0";
const US_MODEL_ID: &str = "us.anthropic.claude-opus-4-8";
const EU_ENDPOINT: &str = "https://bedrock-runtime.eu-central-1.amazonaws.com";
const US_ENDPOINT: &str = "https://bedrock-runtime.us-east-1.amazonaws.com";

fn get_model(provider: &str, id: &str) -> Model {
    assert_eq!(provider, "amazon-bedrock");
    Model {
        id: id.to_owned(),
        provider: provider.to_owned(),
        name: None,
        base_url: Some(
            match id {
                EU_MODEL_ID => EU_ENDPOINT,
                _ => US_ENDPOINT,
            }
            .to_owned(),
        ),
        max_tokens: 128_000,
        reasoning: true,
        thinking_level_map: HashMap::new(),
    }
}

fn options() -> BedrockOptions {
    BedrockOptions {
        cache_retention: Some(CacheRetention::None),
        ..BedrockOptions::default()
    }
}

fn env(pairs: &[(&str, &str)]) -> ProviderEnv {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn capture_client_config(
    model: &Model,
    options: &BedrockOptions,
    ambient: &ProviderEnv,
) -> zedflow_ai::api::bedrock_converse_stream::BedrockClientConfig {
    resolve_bedrock_client_config_with_env(model, options, ambient)
}

#[test]
fn assigns_eu_central_1_runtime_urls_to_built_in_eu_inference_profiles() {
    let model = get_model("amazon-bedrock", EU_MODEL_ID);

    assert_eq!(model.base_url.as_deref(), Some(EU_ENDPOINT));
}

#[test]
fn does_not_pin_standard_aws_endpoints_when_aws_region_is_configured() {
    let model = get_model("amazon-bedrock", US_MODEL_ID);
    let ambient = env(&[("AWS_REGION", "us-east-2")]);

    let config = capture_client_config(&model, &options(), &ambient);

    assert_eq!(config.region.as_deref(), Some("us-east-2"));
    assert_eq!(config.endpoint, None);
}

#[test]
fn derives_region_from_a_built_in_eu_endpoint_when_no_region_or_profile_is_configured() {
    let model = get_model("amazon-bedrock", EU_MODEL_ID);

    let config = capture_client_config(&model, &options(), &ProviderEnv::new());

    assert_eq!(config.endpoint.as_deref(), Some(EU_ENDPOINT));
    assert_eq!(config.region.as_deref(), Some("eu-central-1"));
}

#[test]
fn handles_missing_regions_for_explicit_scoped_and_ambient_profiles() {
    let model = get_model("amazon-bedrock", EU_MODEL_ID);
    let mut bedrock_profile = options();
    bedrock_profile.profile = Some("bedrock-profile".to_owned());

    let mut config = capture_client_config(&model, &bedrock_profile, &ProviderEnv::new());

    assert_eq!(config.profile.as_deref(), Some("bedrock-profile"));
    assert_eq!(config.endpoint.as_deref(), Some(EU_ENDPOINT));
    assert_eq!(config.region.as_deref(), Some("eu-central-1"));

    let mut scoped_profile = options();
    scoped_profile.env = env(&[("AWS_PROFILE", "scoped-bedrock-profile")]);
    config = capture_client_config(&model, &scoped_profile, &ProviderEnv::new());

    assert_eq!(config.profile.as_deref(), Some("scoped-bedrock-profile"));
    assert_eq!(config.endpoint.as_deref(), Some(EU_ENDPOINT));
    assert_eq!(config.region.as_deref(), Some("eu-central-1"));

    config = capture_client_config(
        &model,
        &options(),
        &env(&[("AWS_PROFILE", "ambient-bedrock-profile")]),
    );

    assert_eq!(config.profile.as_deref(), Some("ambient-bedrock-profile"));
    assert_eq!(config.endpoint, None);
    assert_eq!(config.region, None);
}

#[test]
fn still_passes_custom_bedrock_endpoints_through_to_the_sdk_client() {
    let mut model = get_model("amazon-bedrock", US_MODEL_ID);
    model.base_url = Some("https://bedrock-vpc.example.com".to_owned());
    let ambient = env(&[("AWS_REGION", "us-west-2")]);

    let config = capture_client_config(&model, &options(), &ambient);

    assert_eq!(
        config.endpoint.as_deref(),
        Some("https://bedrock-vpc.example.com")
    );
    assert_eq!(config.region.as_deref(), Some("us-west-2"));
}

#[test]
fn extracts_region_from_inference_profile_arn_regardless_of_aws_region() {
    let mut model = get_model("amazon-bedrock", US_MODEL_ID);
    model.id =
        "arn:aws:bedrock:us-west-2:123456789012:application-inference-profile/abc123".to_owned();
    let ambient = env(&[("AWS_REGION", "us-east-1")]);

    let config = capture_client_config(&model, &options(), &ambient);

    assert_eq!(config.region.as_deref(), Some("us-west-2"));
}

#[test]
fn extracts_region_from_gov_cloud_inference_profile_arn() {
    let mut model = get_model("amazon-bedrock", US_MODEL_ID);
    model.id =
        "arn:aws-us-gov:bedrock:us-gov-west-1:123456789012:application-inference-profile/abc123"
            .to_owned();
    let ambient = env(&[("AWS_REGION", "us-east-1")]);

    let config = capture_client_config(&model, &options(), &ambient);

    assert_eq!(config.region.as_deref(), Some("us-gov-west-1"));
}
