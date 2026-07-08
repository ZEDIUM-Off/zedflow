//! Lazy OpenRouter Images API entry point ported from Pi.

use zedflow_core::{error::Result, placeholders};

/// Pi image API identifier.
pub type ImagesApi = String;

/// Pi image provider identifier.
pub type ImagesProviderId = String;

/// Image model metadata passed through the lazy OpenRouter Images wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagesModel {
    /// Model identifier.
    pub id: String,
    /// Image API kind used by the model.
    pub api: ImagesApi,
    /// Provider identifier used by the model.
    pub provider: ImagesProviderId,
}

impl ImagesModel {
    /// Creates image model metadata for lazy image provider calls.
    #[must_use]
    pub fn new(id: impl Into<String>, api: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            api: api.into(),
            provider: provider.into(),
        }
    }
}

/// Image generation request context placeholder.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts ImagesContext`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `carry text and image input content through openrouterImagesApi to the loaded generateImages implementation without mutation`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesContext;

/// Image generation options placeholder.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts ImagesOptions`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `carry API key, abort signal, callbacks, headers, timeout, retry, and metadata options through openrouterImagesApi to the loaded generateImages implementation without mutation`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesOptions;

/// Assistant image response placeholder.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts AssistantImages`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `represent generated text and base64 image output, response id, usage, stop reason, error message, and timestamp returned by generateImages`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssistantImages;

/// Provider image generation functions returned by Pi image API modules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderImages;

impl ProviderImages {
    /// Calls the lazy OpenRouter Images `generateImages` implementation.
    ///
    /// PORT PLACEHOLDER:
    /// Original dependency: `references/pi/packages/ai/src/api/openrouter-images.ts`.
    /// Reason: no Rust replacement selected yet.
    /// Required behavior: `load ./openrouter-images.ts when generateImages is called, cast the model to ImagesModel<"openrouter-images">, forward context and options unchanged, and return the loaded module's generateImages result`.
    /// Replacement decision needed before production use.
    ///
    /// # Errors
    ///
    /// Always returns a port placeholder until `openrouter-images.ts` has a Rust implementation.
    pub async fn generate_images(
        &self,
        _model: &ImagesModel,
        _context: &ImagesContext,
        _options: Option<&ImagesOptions>,
    ) -> Result<AssistantImages> {
        placeholders::unsupported(
            "references/pi/packages/ai/src/api/openrouter-images.ts",
            "load ./openrouter-images.ts when generateImages is called, cast the model to ImagesModel<\"openrouter-images\">, forward context and options unchanged, and return the loaded module's generateImages result",
        )
    }
}

/// Returns the lazy OpenRouter Images provider.
///
/// Like Pi's `openrouterImagesApi`, construction does not perform provider setup or network I/O.
#[must_use]
pub const fn openrouter_images_api() -> ProviderImages {
    ProviderImages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_is_lazy_and_does_not_touch_provider() {
        let provider = openrouter_images_api();

        assert_eq!(provider, ProviderImages);
    }
}
