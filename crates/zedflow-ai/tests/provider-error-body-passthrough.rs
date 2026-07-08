//! Port of Pi `packages/ai/test/provider-error-body-passthrough.test.ts`.
//!
//! The source Vitest suite mocks OpenAI's JavaScript SDK so OpenRouter image generation
//! receives a 403 `APIError` with an opaque message and a parsed body on `.error`. The
//! Rust OpenRouter image transport is still a `request-capture blocker` for that SDK/client and
//! has no injectable fake transport yet, so the parity test is compiled but ignored.

use zedflow_ai::api::openrouter_images::{
    AssistantImages, ImagesContent, ImagesContext, ImagesModel, ImagesOptions,
    ImagesOutputModality, ImagesStopReason, ProviderHeaders, UsageCostRates,
};

const BLOCKER: &str = "OpenRouter image generation still depends on the unselected OpenAI Chat Completions Rust client; unignore when the transport is implemented or injectable so a fake 403 APIError body can be routed through generate_images.";

#[test]
#[ignore = "OpenRouter image transport is not implemented/injectable; see BLOCKER"]
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
    let _ = (model, context, options);
    panic!("{BLOCKER}");
}
