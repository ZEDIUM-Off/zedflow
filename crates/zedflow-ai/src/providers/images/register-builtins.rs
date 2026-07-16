//! Builtin image API provider registration ported from Pi's
//! `packages/ai/src/providers/images/register-builtins.ts`.

use std::sync::{Arc, Once};

use crate::api::openrouter_images;
use crate::images_api_registry::{
    ImagesApiProvider, ImagesContext, ImagesModel, ImagesOptions, ImagesResult,
    register_images_api_provider,
};

/// Production OpenRouter image generation wrapper registered as Pi's builtin image API provider.
pub async fn generate_images_openrouter(
    model: ImagesModel,
    context: ImagesContext,
    options: Option<ImagesOptions>,
) -> ImagesResult<crate::api::openrouter_images::AssistantImages> {
    Ok(openrouter_images::generate_images(&model, &context, options.as_ref()).await)
}

/// Register Pi's builtin image API providers.
pub fn register_built_in_images_api_providers() {
    register_images_api_provider(
        ImagesApiProvider {
            api: "openrouter-images".to_string(),
            generate_images: Arc::new(|model, context, options| {
                Box::pin(generate_images_openrouter(model, context, options))
            }),
        },
        None,
    );
}

static REGISTER_BUILT_INS: Once = Once::new();

/// Ensure builtins are registered before the public image entrypoint resolves a provider.
pub(crate) fn ensure_built_in_images_api_providers() {
    REGISTER_BUILT_INS.call_once(register_built_in_images_api_providers);
}
