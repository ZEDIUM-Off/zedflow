//! Port of Pi `packages/ai/test/images.test.ts`.
//!
//! The source Vitest suite is an OpenRouter live-provider E2E test gated by
//! OpenRouter credentials. These cases use the shared live-capability gate.

mod common;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use zedflow_ai::api::openrouter_images::{
    AssistantImages, ImagesContent, ImagesContext, ImagesModel as ApiImagesModel, ImagesOptions,
    ImagesOutputModality, ImagesStopReason, ProviderHeaders, UsageCostRates, generate_images,
};
use zedflow_ai::image_models::{KnownImagesProvider, get_image_model};
use zedflow_ai::image_models_generated::{ImageModel, ImageModelContent};

const OPENROUTER_GEMINI_FLASH_IMAGE: &str = "google/gemini-2.5-flash-image";

#[test]
fn openrouter_images_generates_basic_image_live_parity() {
    if !openrouter_images_live_ready() {
        return;
    }
    let catalog_model = source_catalog_model();
    let model = api_model(catalog_model);
    let context = ImagesContext {
        input: vec![ImagesContent::Text {
            text: "Generate a simple red circle on a plain white background. No text.".into(),
        }],
    };

    let response = run_live_image_generation(&model, context);

    assert_eq!(
        response.stop_reason,
        ImagesStopReason::Stop,
        "Error: {}",
        response.error_message.as_deref().unwrap_or("")
    );
    assert!(response.error_message.is_none());
    assert!(
        response
            .output
            .iter()
            .any(|item| matches!(item, ImagesContent::Image { .. }))
    );
    assert!(response.timestamp > 0);
}

#[test]
fn openrouter_images_handles_text_plus_image_output_live_parity() {
    if !openrouter_images_live_ready() {
        return;
    }
    let catalog_model = source_catalog_model();
    if !catalog_model.output.contains(&ImageModelContent::Text) {
        return;
    }
    let model = api_model(catalog_model);
    let context = ImagesContext {
        input: vec![ImagesContent::Text {
            text: "Generate a red circle and include a brief description of the image.".into(),
        }],
    };

    let response = run_live_image_generation(&model, context);

    assert_eq!(
        response.stop_reason,
        ImagesStopReason::Stop,
        "Error: {}",
        response.error_message.as_deref().unwrap_or("")
    );
    assert!(
        response
            .output
            .iter()
            .any(|item| matches!(item, ImagesContent::Image { .. }))
    );
    assert!(
        response.output.iter().any(|item| {
            matches!(item, ImagesContent::Text { text } if !text.trim().is_empty())
        })
    );
}

#[test]
fn openrouter_images_handles_image_input_live_parity() {
    if !openrouter_images_live_ready() {
        return;
    }
    let catalog_model = source_catalog_model();
    if !catalog_model.input.contains(&ImageModelContent::Image) {
        return;
    }
    let image_data = STANDARD.encode(include_bytes!(
        "../../../references/pi/packages/ai/test/data/red-circle.png"
    ));
    let model = api_model(catalog_model);
    let context = ImagesContext {
        input: vec![
            ImagesContent::Text {
                text: "Create a variation of this image with a blue background.".into(),
            },
            ImagesContent::Image {
                mime_type: "image/png".into(),
                data: image_data,
            },
        ],
    };

    let response = run_live_image_generation(&model, context);

    assert_eq!(
        response.stop_reason,
        ImagesStopReason::Stop,
        "Error: {}",
        response.error_message.as_deref().unwrap_or("")
    );
    assert!(
        response
            .output
            .iter()
            .any(|item| matches!(item, ImagesContent::Image { .. }))
    );
}

fn source_catalog_model() -> &'static ImageModel {
    get_image_model(
        KnownImagesProvider::Openrouter,
        OPENROUTER_GEMINI_FLASH_IMAGE,
    )
    .expect("source image model should stay represented")
}

fn api_model(model: &ImageModel) -> ApiImagesModel {
    ApiImagesModel {
        id: model.id.into(),
        api: model.api.into(),
        provider: model.provider.into(),
        base_url: model.base_url.into(),
        headers: ProviderHeaders::default(),
        output: model
            .output
            .iter()
            .map(|content| match content {
                ImageModelContent::Text => ImagesOutputModality::Text,
                ImageModelContent::Image => ImagesOutputModality::Image,
            })
            .collect(),
        cost: UsageCostRates {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
    }
}

fn openrouter_images_live_ready() -> bool {
    if let Some(message) = common::live_credentials::openrouter().skip_message() {
        eprintln!("{message}");
        return false;
    }
    true
}

fn run_live_image_generation(model: &ApiImagesModel, context: ImagesContext) -> AssistantImages {
    let api_key = common::live_credentials::api_key("openrouter")
        .unwrap_or_else(|| panic!("missing OpenRouter credential after capability check"));
    let options = ImagesOptions {
        api_key: Some(api_key),
        ..ImagesOptions::default()
    };

    futures::executor::block_on(generate_images(model, &context, Some(&options)))
}
