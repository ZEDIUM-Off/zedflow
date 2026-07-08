//! Image model registry helpers ported from Pi's `image-models.ts`.

use std::fmt;

use crate::image_models_generated::{IMAGE_MODELS, ImageProviderModels};
pub use crate::image_models_generated::{
    ImageModel as ImagesModel, ImageModelContent as ImageModality, ImageModelCost,
};

/// Pi image API identifier.
pub type ImagesApi = String;

/// Pi image provider identifier.
pub type ImagesProviderId = String;

/// Built-in image providers known to Pi's generated image catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownImagesProvider {
    /// OpenRouter image models.
    Openrouter,
}

impl KnownImagesProvider {
    /// Returns the provider id used by Pi.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Openrouter => "openrouter",
        }
    }

    /// Parses a generated provider id into a known image provider.
    #[must_use]
    pub const fn from_id(id: &str) -> Option<Self> {
        match id.as_bytes() {
            b"openrouter" => Some(Self::Openrouter),
            _ => None,
        }
    }
}

impl fmt::Display for KnownImagesProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Returns a model from Pi's generated image catalog.
#[must_use]
pub fn get_image_model(
    provider: KnownImagesProvider,
    model_id: &str,
) -> Option<&'static ImagesModel> {
    provider_models(provider)?
        .models
        .iter()
        .find(|model| model.id == model_id)
}

/// Returns providers from Pi's generated image model registry in registry order.
#[must_use]
pub fn get_image_providers() -> Vec<KnownImagesProvider> {
    IMAGE_MODELS
        .iter()
        .filter_map(|entry| KnownImagesProvider::from_id(entry.provider))
        .collect()
}

/// Returns all models for a provider from Pi's generated image catalog.
#[must_use]
pub fn get_image_models(provider: KnownImagesProvider) -> Vec<&'static ImagesModel> {
    provider_models(provider)
        .map(|entry| entry.models.iter().collect())
        .unwrap_or_default()
}

fn provider_models(provider: KnownImagesProvider) -> Option<&'static ImageProviderModels> {
    IMAGE_MODELS
        .iter()
        .find(|entry| entry.provider == provider.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_come_from_generated_catalog() {
        assert_eq!(get_image_providers(), vec![KnownImagesProvider::Openrouter]);
    }

    #[test]
    fn model_lookup_reads_generated_catalog() {
        let model = get_image_model(KnownImagesProvider::Openrouter, "openrouter/auto")
            .expect("openrouter auto model should be generated");

        assert_eq!(model.api, "openrouter-images");
        assert_eq!(model.provider, "openrouter");
    }
}
