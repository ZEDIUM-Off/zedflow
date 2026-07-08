//! Builtin image API provider registration ported from Pi's
//! `packages/ai/src/providers/images/register-builtins.ts`.

use std::sync::Arc;

use crate::images_api_registry::{
    AssistantImages, ImagesApiProvider, ImagesApiRegistryError, ImagesContext, ImagesModel,
    ImagesOptions, register_images_api_provider,
};

fn create_lazy_load_error_images(_model: &ImagesModel, _error: &str) -> AssistantImages {
    // PORT PLACEHOLDER:
    // Original dependency: `references/pi/packages/ai/src/types.ts AssistantImages`.
    // Reason: the current Rust registry response only stores generated image payloads.
    // Required behavior: preserve api, provider, model, empty output, stopReason="error",
    // errorMessage, and timestamp when lazy loading fails.
    // Replacement decision needed before production use.
    AssistantImages { images: Vec::new() }
}

/// Lazy OpenRouter image generation wrapper registered as Pi's builtin image API provider.
pub fn generate_images_openrouter(
    model: &ImagesModel,
    _context: &ImagesContext,
    _options: Option<&ImagesOptions>,
) -> Result<AssistantImages, ImagesApiRegistryError> {
    Ok(create_lazy_load_error_images(
        model,
        "OpenRouter image provider lazy loading is not wired in Rust yet",
    ))
}

/// Register Pi's builtin image API providers.
///
/// This mirrors the TypeScript module's exported registration function. Rust has no import-time
/// side effect here; callers must invoke this function when the module is wired into `lib.rs`.
pub fn register_built_in_images_api_providers() {
    register_images_api_provider(
        ImagesApiProvider {
            api: "openrouter-images".to_string(),
            generate_images: Arc::new(generate_images_openrouter),
        },
        None,
    );
}
