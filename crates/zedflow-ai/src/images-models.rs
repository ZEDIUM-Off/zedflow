//! Runtime image provider collection ported from Pi's `packages/ai/src/images-models.ts`.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Image API identifier.
pub type ImagesApi = String;

/// Minimal image model shape used by the image model collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagesModel {
    /// Model id.
    pub id: String,
    /// API id.
    pub api: ImagesApi,
    /// Owning provider id.
    pub provider: String,
    /// Optional provider base URL override.
    pub base_url: Option<String>,
}

/// Image generation context.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesContext {
    /// Input payloads.
    pub input: Vec<String>,
}

/// Image generation options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesOptions {
    /// Explicit API key; wins over resolved auth.
    pub api_key: Option<String>,
    /// Explicit request headers; win over resolved auth per key.
    pub headers: HashMap<String, String>,
    /// Explicit request environment values; win over resolved auth per key.
    pub env: HashMap<String, String>,
}

/// Image generation result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssistantImages {
    /// API id.
    pub api: ImagesApi,
    /// Provider id.
    pub provider: String,
    /// Model id.
    pub model: String,
    /// Output payloads.
    pub output: Vec<String>,
    /// Stop reason.
    pub stop_reason: String,
    /// Optional error message.
    pub error_message: Option<String>,
}

/// Provider auth marker. Later auth rows can replace this with richer shared auth types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderAuth {
    /// Whether this provider can be used without per-request credentials.
    pub ambient: bool,
}

/// Resolved auth values applied to image requests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthResult {
    /// Optional API key.
    pub api_key: Option<String>,
    /// Optional base URL.
    pub base_url: Option<String>,
    /// Headers from auth resolution.
    pub headers: HashMap<String, String>,
    /// Environment values from auth resolution.
    pub env: HashMap<String, String>,
}

/// Error kind used by image model operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsError {
    /// Error category, e.g. `provider` or `model_source`.
    pub kind: String,
    /// Human-readable message.
    pub message: String,
}

impl ModelsError {
    /// Create a model error.
    #[must_use]
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ModelsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ModelsError {}

/// Boxed future used by image provider callbacks.
pub type ImagesFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Image generation callback.
pub type GenerateImages = dyn Fn(ImagesModel, ImagesContext, Option<ImagesOptions>) -> ImagesFuture<AssistantImages>
    + Send
    + Sync;

/// Dynamic model refresh callback.
pub type RefreshImagesModels =
    dyn Fn() -> ImagesFuture<Result<Vec<ImagesModel>, ModelsError>> + Send + Sync;

/// An image-generation provider.
#[derive(Clone)]
pub struct ImagesProvider {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Provider auth semantics.
    pub auth: ProviderAuth,
    models: Vec<ImagesModel>,
    refresh_models: Option<Arc<RefreshImagesModels>>,
    generate_images: Arc<GenerateImages>,
}

impl ImagesProvider {
    /// Current known models. Panics from custom providers are not represented in this Rust port.
    #[must_use]
    pub fn get_models(&self) -> Vec<ImagesModel> {
        self.models.clone()
    }

    /// Replace current known models.
    pub fn set_models(&mut self, models: Vec<ImagesModel>) {
        self.models = models;
    }

    /// Refresh dynamic models, if supported.
    pub async fn refresh_models(&mut self) -> Result<(), ModelsError> {
        if let Some(refresh_models) = &self.refresh_models {
            self.models = refresh_models().await?;
        }
        Ok(())
    }

    /// Generate images with this provider.
    pub async fn generate_images(
        &self,
        model: ImagesModel,
        context: ImagesContext,
        options: Option<ImagesOptions>,
    ) -> AssistantImages {
        (self.generate_images)(model, context, options).await
    }
}

/// Mutable runtime collection of image-generation providers.
#[derive(Default, Clone)]
pub struct ImagesModels {
    providers: HashMap<String, ImagesProvider>,
}

impl ImagesModels {
    /// Upsert/replace a provider by id.
    pub fn set_provider(&mut self, provider: ImagesProvider) {
        self.providers.insert(provider.id.clone(), provider);
    }

    /// Delete a provider.
    pub fn delete_provider(&mut self, id: &str) {
        self.providers.remove(id);
    }

    /// Clear all providers.
    pub fn clear_providers(&mut self) {
        self.providers.clear();
    }

    /// Get all providers.
    #[must_use]
    pub fn get_providers(&self) -> Vec<ImagesProvider> {
        self.providers.values().cloned().collect()
    }

    /// Get a provider by id.
    #[must_use]
    pub fn get_provider(&self, id: &str) -> Option<&ImagesProvider> {
        self.providers.get(id)
    }

    /// Sync read of last-known models from one provider or all providers.
    #[must_use]
    pub fn get_models(&self, provider: Option<&str>) -> Vec<ImagesModel> {
        match provider {
            Some(provider) => self
                .providers
                .get(provider)
                .map(ImagesProvider::get_models)
                .unwrap_or_default(),
            None => self
                .providers
                .values()
                .flat_map(ImagesProvider::get_models)
                .collect(),
        }
    }

    /// Sync runtime model lookup against last-known lists.
    #[must_use]
    pub fn get_model(&self, provider: &str, id: &str) -> Option<ImagesModel> {
        self.get_models(Some(provider))
            .into_iter()
            .find(|model| model.id == id)
    }

    /// Refresh one provider or all providers. All-provider refresh is best-effort.
    pub async fn refresh(&mut self, provider: Option<&str>) -> Result<(), ModelsError> {
        if let Some(provider) = provider {
            if let Some(entry) = self.providers.get_mut(provider) {
                entry.refresh_models().await.map_err(|error| {
                    ModelsError::new(
                        "model_source",
                        format!("Model refresh failed for {provider}: {error}"),
                    )
                })?;
            }
            return Ok(());
        }

        for entry in self.providers.values_mut() {
            let _ = entry.refresh_models().await;
        }
        Ok(())
    }

    /// Resolve request auth. This row keeps auth resolution as a placeholder-free ambient marker.
    pub async fn get_auth(&self, model: &ImagesModel) -> Result<Option<AuthResult>, ModelsError> {
        Ok(self
            .providers
            .get(&model.provider)
            .and_then(|provider| provider.auth.ambient.then_some(AuthResult::default())))
    }

    /// Generate images through the owning provider; failures are returned as an error image result.
    pub async fn generate_images(
        &self,
        model: ImagesModel,
        context: ImagesContext,
        options: Option<ImagesOptions>,
    ) -> AssistantImages {
        match self.providers.get(&model.provider) {
            Some(provider) => provider.generate_images(model, context, options).await,
            None => {
                let provider = model.provider;
                AssistantImages {
                    api: model.api,
                    provider: provider.clone(),
                    model: model.id,
                    output: Vec::new(),
                    stop_reason: "error".to_string(),
                    error_message: Some(format!("Unknown provider: {provider}")),
                }
            }
        }
    }
}

/// Create an empty image provider collection.
#[must_use]
pub fn create_images_models() -> ImagesModels {
    ImagesModels::default()
}

/// Options for [`create_images_provider`].
pub struct CreateImagesProviderOptions {
    /// Provider id.
    pub id: String,
    /// Optional display name. Defaults to `id`.
    pub name: Option<String>,
    /// Auth semantics.
    pub auth: ProviderAuth,
    /// Initial model list.
    pub models: Vec<ImagesModel>,
    /// Dynamic model refresh callback.
    pub refresh_models: Option<Arc<RefreshImagesModels>>,
    /// Generation callback.
    pub generate_images: Arc<GenerateImages>,
}

/// Build an image-generation provider from parts.
#[must_use]
pub fn create_images_provider(input: CreateImagesProviderOptions) -> ImagesProvider {
    ImagesProvider {
        id: input.id.clone(),
        name: input.name.unwrap_or(input.id),
        auth: input.auth,
        models: input.models,
        refresh_models: input.refresh_models,
        generate_images: input.generate_images,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, id: &str) -> ImagesModel {
        ImagesModel {
            id: id.to_string(),
            api: "api".to_string(),
            provider: provider.to_string(),
            base_url: None,
        }
    }

    #[test]
    fn provider_collection_finds_models() {
        let mut models = create_images_models();
        models.set_provider(create_images_provider(CreateImagesProviderOptions {
            id: "openrouter".into(),
            name: None,
            auth: ProviderAuth { ambient: true },
            models: vec![model("openrouter", "m1")],
            refresh_models: None,
            generate_images: Arc::new(|model, _, _| {
                Box::pin(async move {
                    AssistantImages {
                        api: model.api,
                        provider: model.provider,
                        model: model.id,
                        output: vec!["ok".into()],
                        stop_reason: "stop".into(),
                        error_message: None,
                    }
                })
            }),
        }));

        assert_eq!(models.get_model("openrouter", "m1").unwrap().id, "m1");
        assert!(models.get_model("missing", "m1").is_none());
    }

    #[test]
    fn unknown_provider_returns_error_result() {
        let result = futures::executor::block_on(create_images_models().generate_images(
            model("missing", "m1"),
            ImagesContext::default(),
            None,
        ));

        assert_eq!(result.stop_reason, "error");
        assert_eq!(
            result.error_message,
            Some("Unknown provider: missing".into())
        );
    }
}
