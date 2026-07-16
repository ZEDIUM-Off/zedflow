//! Port of Pi `packages/ai/test/provider-error-body-passthrough.test.ts`.

mod common;

use std::collections::HashMap;
use std::error::Error;
use std::io;

use common::http_capture::{CapturedRequest, FixtureResponse, HttpCapture};
use serde_json::json;
use zedflow_ai::api::bedrock_converse_stream;
use zedflow_ai::api::openrouter_images::{
    AssistantImages, ImagesContent, ImagesContext, ImagesModel, ImagesOptions,
    ImagesOutputModality, ImagesStopReason, ProviderHeaders, UsageCostRates,
};
use zedflow_ai::utils::error_body::{
    ProviderErrorInput, ProviderHttpErrorParts, ProviderServiceError, SdkErrorShape,
    format_provider_error, normalize_provider_error,
};

#[test]
fn bedrock_preserves_non_json_body_status_message_and_metadata() {
    let error = bedrock_converse_stream::bedrock_service_error(
        502,
        "upstream proxy unavailable",
        HashMap::from([("x-amzn-requestid".to_owned(), "request-502".to_owned())]),
    );

    assert_eq!(error.http.normalized.status, Some(502.0));
    assert_eq!(
        error.http.normalized.message,
        "502 status code (response body preserved)"
    );
    assert_eq!(
        error.http.normalized.body.as_deref(),
        Some("upstream proxy unavailable")
    );
    assert_eq!(
        error
            .http
            .headers
            .get("x-amzn-requestid")
            .map(String::as_str),
        Some("request-502")
    );
    assert_eq!(error.to_string(), "502: upstream proxy unavailable");
}

#[test]
fn canonical_provider_error_retains_its_source_chain() {
    let error = ProviderServiceError::with_source(
        ProviderHttpErrorParts::new("Bedrock request failed"),
        io::Error::other("socket closed"),
    );

    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("socket closed")
    );
}

#[test]
fn surfaces_the_http_body_reason_instead_of_the_opaque_sdk_message_openrouter_images() {
    let model = ImagesModel {
        id: "black-forest-labs/flux.2-pro".to_string(),
        api: "openrouter-images".to_string(),
        provider: "openrouter".to_string(),
        base_url: "https://openrouter.ai/api/v1".to_string(),
        headers: ProviderHeaders::default(),
        output: vec![ImagesOutputModality::Image],
        cost: UsageCostRates {
            input: 0.015,
            output: 0.03,
            cache_read: 0.0,
            cache_write: 0.0,
        },
    };
    let context = ImagesContext {
        input: vec![ImagesContent::Text {
            text: "Generate a dog".to_string(),
        }],
    };
    let options = ImagesOptions {
        api_key: Some("test".to_string()),
        ..ImagesOptions::default()
    };

    let output = generate_images_with_fake_openai_api_error(&model, &context, &options);

    assert_eq!(output.stop_reason, ImagesStopReason::Error);
    let error_message = output.error_message.as_deref().unwrap_or_default();
    assert!(error_message.contains("403"));
    assert!(error_message.contains("blocked by gateway WAF"));
    assert_ne!(error_message, "403 status code (no body)");
}

fn generate_images_with_fake_openai_api_error(
    model: &ImagesModel,
    context: &ImagesContext,
    options: &ImagesOptions,
) -> AssistantImages {
    let mut output = AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        output: Vec::new(),
        response_id: None,
        usage: None,
        stop_reason: ImagesStopReason::Error,
        error_message: None,
        timestamp: 0,
    };
    let request = zedflow_ai::api::openrouter_images::build_request(model, context, Some(options));
    let capture = HttpCapture::new([FixtureResponse::text(403, "blocked by gateway WAF")]);
    capture
        .request(
            CapturedRequest::new("POST", format!("{}/chat/completions", request.base_url))
                .json_body(&request.body),
        )
        .expect("fixture response should be queued");
    let captured = capture.next_request().expect("request should be captured");
    assert_eq!(captured.method, "POST");
    assert_eq!(
        captured
            .body_json()
            .and_then(|body| body.get("model").cloned()),
        Some(json!(model.id))
    );

    let normalized = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "403 status code (no body)".to_owned(),
        status: Some(403.0),
        error: Some(json!({ "error": "blocked by gateway WAF" })),
        ..SdkErrorShape::default()
    }));
    output.error_message = Some(format_provider_error(&normalized, None));
    output
}
