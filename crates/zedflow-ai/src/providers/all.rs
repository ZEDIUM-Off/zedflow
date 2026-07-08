//! Built-in provider helpers ported from Pi's `packages/ai/src/providers/all.ts`.

use crate::image_models::{KnownImagesProvider, get_image_models};
use crate::images_models::{
    AssistantImages, CreateImagesProviderOptions, ImagesModel, ImagesProvider, ProviderAuth,
    create_images_models, create_images_provider,
};
use crate::models::{Model, Models, Provider, create_models};
use crate::models_generated::BUILTIN_PROVIDERS;

/// Typed read of the generated built-in catalog. Provider model rows are ported separately,
/// so this returns `None` until those rows populate model metadata.
#[must_use]
pub fn get_builtin_model(_provider: &str, _model_id: &str) -> Option<Model> {
    None
}

/// Built-in provider ids from the generated catalog.
#[must_use]
pub fn get_builtin_providers() -> Vec<&'static str> {
    BUILTIN_PROVIDERS.to_vec()
}

/// Built-in models for a provider. Detailed provider model catalogs are owned by provider rows.
#[must_use]
pub fn get_builtin_models(_provider: &str) -> Vec<Model> {
    Vec::new()
}

/// All built-in providers, freshly constructed. Concrete factories are ported by provider rows.
#[must_use]
pub fn builtin_providers() -> Vec<Provider> {
    Vec::new()
}

/// A `Models` collection with every built-in provider registered.
#[must_use]
pub fn builtin_models() -> Models {
    let mut models = create_models();
    for provider in builtin_providers() {
        models.set_provider(provider);
    }
    models
}

/// All built-in image-generation providers, freshly constructed.
#[must_use]
pub fn builtin_images_providers() -> Vec<ImagesProvider> {
    vec![create_images_provider(CreateImagesProviderOptions {
        id: "openrouter".into(),
        name: Some("OpenRouter".into()),
        auth: ProviderAuth::default(),
        models: get_image_models(KnownImagesProvider::Openrouter)
            .into_iter()
            .map(|model| ImagesModel {
                api: model.api.to_string(),
                id: model.id.to_string(),
                provider: "openrouter".to_string(),
                base_url: None,
            })
            .collect(),
        refresh_models: None,
        generate_images: std::sync::Arc::new(|_, _, _| {
            Box::pin(async { AssistantImages::default() })
        }),
    })]
}

/// An `ImagesModels` collection with every built-in image-generation provider registered.
#[must_use]
pub fn builtin_images_models() -> crate::images_models::ImagesModels {
    let mut models = create_images_models();
    for provider in builtin_images_providers() {
        models.set_provider(provider);
    }
    models
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_builtin_provider_keys() {
        assert!(get_builtin_providers().contains(&"openai"));
    }
}
