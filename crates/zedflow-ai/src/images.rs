//! Image generation entrypoint ported from Pi's `packages/ai/src/images.ts`.

use crate::images_api_registry::{
    AssistantImages, ImagesContext, ImagesModel, ImagesOptions, get_images_api_provider,
};
use crate::providers::images::register_builtins::ensure_built_in_images_api_providers;

/// Resolves an image API provider or returns Pi's missing-provider error text.
pub fn resolve_images_api_provider(
    api: &str,
) -> Result<crate::images_api_registry::ImagesApiProviderInternal, String> {
    ensure_built_in_images_api_providers();
    get_images_api_provider(api).ok_or_else(|| format!("No API provider registered for api: {api}"))
}

/// Generate images through the registered API provider for `model.api`.
pub async fn generate_images(
    model: &ImagesModel,
    context: &ImagesContext,
    options: Option<&ImagesOptions>,
) -> Result<AssistantImages, String> {
    let provider = resolve_images_api_provider(&model.api)?;
    (provider.generate_images)(model.clone(), context.clone(), options.cloned())
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::api::openrouter_images::ImagesStopReason;
    use crate::images_api_registry::{ImagesApiProvider, register_images_api_provider};

    fn model(api: &str) -> ImagesModel {
        ImagesModel {
            id: "model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            headers: Default::default(),
            output: Vec::new(),
            cost: Default::default(),
        }
    }

    #[test]
    fn errors_when_provider_missing() {
        let error = futures::executor::block_on(generate_images(
            &model("missing"),
            &ImagesContext::default(),
            None,
        ))
        .expect_err("missing provider should fail");

        assert_eq!(error, "No API provider registered for api: missing");
    }

    #[test]
    fn dispatches_registered_provider() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "images-test".into(),
                generate_images: Arc::new(|model, _, _| {
                    Box::pin(async move {
                        Ok(AssistantImages {
                            api: model.api,
                            provider: model.provider,
                            model: model.id,
                            output: Vec::new(),
                            response_id: None,
                            usage: None,
                            stop_reason: ImagesStopReason::Stop,
                            error_message: None,
                            timestamp: 1,
                        })
                    })
                }),
            },
            None,
        );

        let images = futures::executor::block_on(generate_images(
            &model("images-test"),
            &ImagesContext::default(),
            None,
        ))
        .expect("registered provider works");

        assert_eq!(images.stop_reason, ImagesStopReason::Stop);
    }
}
