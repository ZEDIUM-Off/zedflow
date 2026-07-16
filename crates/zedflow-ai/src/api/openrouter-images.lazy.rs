//! Static OpenRouter Images API entry point ported from Pi's lazy module.
//!
//! Rust links the implementation statically, so this facade preserves Pi's provider shape without
//! maintaining a second image type universe.

pub use super::openrouter_images::{AssistantImages, ImagesContext, ImagesModel, ImagesOptions};

/// Provider image generation functions returned by Pi image API modules.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderImages;

impl ProviderImages {
    /// Calls the production OpenRouter Images implementation.
    pub async fn generate_images(
        &self,
        model: &ImagesModel,
        context: &ImagesContext,
        options: Option<&ImagesOptions>,
    ) -> AssistantImages {
        super::openrouter_images::generate_images(model, context, options).await
    }
}

/// Returns the OpenRouter Images provider.
#[must_use]
pub const fn openrouter_images_api() -> ProviderImages {
    ProviderImages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_is_static_and_does_not_touch_provider() {
        assert_eq!(openrouter_images_api(), ProviderImages);
    }
}
