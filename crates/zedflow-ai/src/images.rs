//! Image generation entrypoint ported from Pi's `packages/ai/src/images.ts`.

use crate::images_api_registry::{
    AssistantImages, ImagesContext, ImagesModel, ImagesOptions, get_images_api_provider,
};

/// Resolves an image API provider or returns Pi's missing-provider error text.
pub fn resolve_images_api_provider(
    api: &str,
) -> Result<crate::images_api_registry::ImagesApiProviderInternal, String> {
    get_images_api_provider(api).ok_or_else(|| format!("No API provider registered for api: {api}"))
}

/// Generate images through the registered API provider for `model.api`.
pub fn generate_images(
    model: &ImagesModel,
    context: &ImagesContext,
    options: Option<&ImagesOptions>,
) -> Result<AssistantImages, String> {
    let provider = resolve_images_api_provider(&model.api)?;
    (provider.generate_images)(model, context, options).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::images_api_registry::{ImagesApiProvider, register_images_api_provider};

    #[test]
    fn errors_when_provider_missing() {
        let error = generate_images(
            &ImagesModel {
                api: "missing".into(),
                id: "model".into(),
            },
            &ImagesContext::default(),
            None,
        )
        .expect_err("missing provider should fail");

        assert_eq!(error, "No API provider registered for api: missing");
    }

    #[test]
    fn dispatches_registered_provider() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "images-test".into(),
                generate_images: Arc::new(|_, _, _| {
                    Ok(AssistantImages {
                        images: vec!["ok".into()],
                    })
                }),
            },
            None,
        );

        let images = generate_images(
            &ImagesModel {
                api: "images-test".into(),
                id: "model".into(),
            },
            &ImagesContext::default(),
            None,
        )
        .expect("registered provider works");

        assert_eq!(images.images, ["ok"]);
    }
}
