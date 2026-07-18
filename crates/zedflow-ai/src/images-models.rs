//! Runtime image provider collection ported from Pi's `packages/ai/src/images-models.ts`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::future::{BoxFuture, FutureExt, Shared, join_all};

use crate::auth::resolve::{
    AuthResolutionOverrides, ModelsError as AuthModelsError, resolve_provider_auth,
};
use crate::auth::types::{
    AuthContext, AuthFuture, AuthModel, AuthProvider, CredentialStore,
    ProviderEnv as AuthProviderEnv, ProviderHeaders as AuthProviderHeaders, ResolvedAuth,
};

pub use crate::auth::types::ProviderAuth;
pub use crate::types::{
    AssistantImages, ImagesApi, ImagesContext, ImagesModel, ImagesOptions, ImagesStopReason,
};

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

impl From<AuthModelsError> for ModelsError {
    fn from(error: AuthModelsError) -> Self {
        Self::new(error.code().as_str(), error.to_string())
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

type SharedRefresh = Shared<BoxFuture<'static, Result<(), ModelsError>>>;

/// An image-generation provider.
#[derive(Clone)]
pub struct ImagesProvider {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Provider auth semantics.
    pub auth: ProviderAuth,
    models: Arc<Mutex<Vec<ImagesModel>>>,
    refresh_models: Option<Arc<RefreshImagesModels>>,
    in_flight_refresh: Arc<Mutex<Option<SharedRefresh>>>,
    generate_images: Arc<GenerateImages>,
}

impl ImagesProvider {
    /// Current known models. Panics from custom providers are not represented in this Rust port.
    #[must_use]
    pub fn get_models(&self) -> Vec<ImagesModel> {
        self.models
            .lock()
            .expect("image models lock poisoned")
            .clone()
    }

    /// Replace current known models.
    pub fn set_models(&self, models: Vec<ImagesModel>) {
        *self.models.lock().expect("image models lock poisoned") = models;
    }

    /// Refresh dynamic models, if supported. Concurrent calls share one fetch.
    pub async fn refresh_models(&self) -> Result<(), ModelsError> {
        let Some(refresh_models) = &self.refresh_models else {
            return Ok(());
        };

        let in_flight = {
            let mut guard = self
                .in_flight_refresh
                .lock()
                .expect("image refresh lock poisoned");
            if let Some(in_flight) = guard.clone() {
                in_flight
            } else {
                let refresh_models = Arc::clone(refresh_models);
                let models = Arc::clone(&self.models);
                let in_flight_refresh = Arc::clone(&self.in_flight_refresh);
                let in_flight = async move {
                    let result = refresh_models().await;
                    if let Ok(next_models) = result.as_ref() {
                        *models.lock().expect("image models lock poisoned") = next_models.clone();
                    }
                    *in_flight_refresh
                        .lock()
                        .expect("image refresh lock poisoned") = None;
                    result.map(|_| ())
                }
                .boxed()
                .shared();
                *guard = Some(in_flight.clone());
                in_flight
            }
        };
        in_flight.await
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
#[derive(Clone)]
pub struct ImagesModels {
    providers: Vec<ImagesProvider>,
    auth_context: Arc<dyn AuthContext>,
    credentials: Arc<dyn CredentialStore>,
}

impl Default for ImagesModels {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            auth_context: Arc::new(DefaultAuthContext),
            credentials: Arc::new(crate::auth::credential_store::InMemoryCredentialStore::new()),
        }
    }
}

#[derive(Debug, Default)]
struct DefaultAuthContext;

impl AuthContext for DefaultAuthContext {
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>> {
        Box::pin(async move {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
    }

    fn file_exists<'a>(&'a self, path: &'a str) -> AuthFuture<'a, bool> {
        Box::pin(async move { default_file_exists(path) })
    }
}

impl ImagesModels {
    /// Upsert/replace a provider by id without moving its insertion slot.
    pub fn set_provider(&mut self, provider: ImagesProvider) {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|entry| entry.id == provider.id)
        {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
    }

    /// Delete a provider.
    pub fn delete_provider(&mut self, id: &str) {
        self.providers.retain(|provider| provider.id != id);
    }

    /// Clear all providers.
    pub fn clear_providers(&mut self) {
        self.providers.clear();
    }

    /// Get all providers in insertion order.
    #[must_use]
    pub fn get_providers(&self) -> Vec<ImagesProvider> {
        self.providers.clone()
    }

    /// Get a provider by id.
    #[must_use]
    pub fn get_provider(&self, id: &str) -> Option<&ImagesProvider> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    /// Sync read of last-known models from one provider or all providers in insertion order.
    #[must_use]
    pub fn get_models(&self, provider: Option<&str>) -> Vec<ImagesModel> {
        if let Some(provider) = provider {
            return self
                .get_provider(provider)
                .map_or_else(Vec::new, ImagesProvider::get_models);
        }
        self.providers
            .iter()
            .flat_map(ImagesProvider::get_models)
            .collect()
    }

    /// Sync runtime model lookup against last-known lists.
    #[must_use]
    pub fn get_model(&self, provider: &str, id: &str) -> Option<ImagesModel> {
        self.get_models(Some(provider))
            .into_iter()
            .find(|model| model.id == id)
    }

    /// Refresh one provider or all providers. All-provider refresh is best-effort.
    pub async fn refresh(&self, provider: Option<&str>) -> Result<(), ModelsError> {
        if let Some(provider) = provider {
            if let Some(entry) = self.get_provider(provider) {
                entry.refresh_models().await.map_err(|error| {
                    if error.kind == "model_source" {
                        error
                    } else {
                        ModelsError::new(
                            "model_source",
                            format!("Model refresh failed for {provider}: {error}"),
                        )
                    }
                })?;
            }
            return Ok(());
        }

        let _ = join_all(self.providers.iter().map(ImagesProvider::refresh_models)).await;
        Ok(())
    }

    /// Resolve request auth for an image model.
    pub async fn get_auth(&self, model: &ImagesModel) -> Result<Option<ResolvedAuth>, ModelsError> {
        let Some(provider) = self.get_provider(&model.provider) else {
            return Ok(None);
        };
        self.resolve_auth(provider, model, None).await
    }

    /// Generate images through the owning provider; failures are returned as an error image result.
    pub async fn generate_images(
        &self,
        model: ImagesModel,
        context: ImagesContext,
        options: Option<ImagesOptions>,
    ) -> AssistantImages {
        match self.get_provider(&model.provider).ok_or_else(|| {
            ModelsError::new("provider", format!("Unknown provider: {}", model.provider))
        }) {
            Ok(provider) => match self.apply_auth(provider, &model, options).await {
                Ok((request_model, request_options)) => {
                    provider
                        .generate_images(request_model, context, request_options)
                        .await
                }
                Err(error) => error_images(&model, error.to_string()),
            },
            Err(error) => error_images(&model, error.message),
        }
    }

    async fn resolve_auth(
        &self,
        provider: &ImagesProvider,
        model: &ImagesModel,
        overrides: Option<&AuthResolutionOverrides>,
    ) -> Result<Option<ResolvedAuth>, ModelsError> {
        resolve_provider_auth(
            &AuthProvider {
                id: provider.id.clone(),
                auth: provider.auth.clone(),
            },
            &auth_model(model),
            self.credentials.as_ref(),
            self.auth_context.as_ref(),
            overrides,
        )
        .await
        .map_err(Into::into)
    }

    async fn apply_auth(
        &self,
        provider: &ImagesProvider,
        model: &ImagesModel,
        options: Option<ImagesOptions>,
    ) -> Result<(ImagesModel, Option<ImagesOptions>), ModelsError> {
        let overrides = auth_overrides(options.as_ref());
        let resolution = self
            .resolve_auth(provider, model, overrides.as_ref())
            .await?;
        Ok(merge_resolved_auth(model, options, resolution))
    }
}

/// Create an empty image provider collection.
#[must_use]
pub fn create_images_models() -> ImagesModels {
    ImagesModels::default()
}

/// Creates an empty image provider collection with a custom auth context.
#[must_use]
pub fn create_images_models_with_auth_context(context: impl AuthContext + 'static) -> ImagesModels {
    create_images_models_with_auth_context_and_credentials(
        context,
        crate::auth::credential_store::InMemoryCredentialStore::new(),
    )
}

/// Creates an empty image provider collection with a custom credential store.
#[must_use]
pub fn create_images_models_with_credentials(
    credentials: impl CredentialStore + 'static,
) -> ImagesModels {
    ImagesModels {
        providers: Vec::new(),
        auth_context: Arc::new(DefaultAuthContext),
        credentials: Arc::new(credentials),
    }
}

/// Creates an empty image provider collection with custom auth context and credentials.
#[must_use]
pub fn create_images_models_with_auth_context_and_credentials(
    context: impl AuthContext + 'static,
    credentials: impl CredentialStore + 'static,
) -> ImagesModels {
    ImagesModels {
        providers: Vec::new(),
        auth_context: Arc::new(context),
        credentials: Arc::new(credentials),
    }
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
        models: Arc::new(Mutex::new(input.models)),
        refresh_models: input.refresh_models,
        in_flight_refresh: Arc::new(Mutex::new(None)),
        generate_images: input.generate_images,
    }
}

fn auth_model(model: &ImagesModel) -> AuthModel {
    AuthModel {
        provider: model.provider.clone(),
        api: model.api.clone(),
        id: model.id.clone(),
        base_url: (!model.base_url.is_empty()).then(|| model.base_url.clone()),
    }
}

fn auth_overrides(options: Option<&ImagesOptions>) -> Option<AuthResolutionOverrides> {
    let options = options?;
    let env = options.env.as_ref().filter(|env| !env.is_empty());
    (options.api_key.is_some() || env.is_some()).then(|| AuthResolutionOverrides {
        api_key: options.api_key.clone(),
        env: env.map(|env| {
            env.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        }),
    })
}

fn merge_resolved_auth(
    model: &ImagesModel,
    options: Option<ImagesOptions>,
    resolution: Option<ResolvedAuth>,
) -> (ImagesModel, Option<ImagesOptions>) {
    let Some(resolution) = resolution else {
        return (model.clone(), options);
    };

    let mut request_model = model.clone();
    if let Some(base_url) = resolution
        .auth
        .base_url
        .filter(|base_url| !base_url.is_empty())
    {
        request_model.base_url = base_url;
    }

    let mut request_options = options.unwrap_or_default();
    if request_options.api_key.is_none() {
        request_options.api_key = resolution.auth.api_key;
    }
    if let Some(headers) = resolution.auth.headers {
        request_options.headers = Some(merge_headers(
            headers,
            request_options.headers.unwrap_or_default(),
        ));
    }
    if let Some(env) = resolution.env {
        request_options.env = Some(merge_env(env, request_options.env.unwrap_or_default()));
    }

    (request_model, Some(request_options))
}

fn merge_headers(
    resolved: AuthProviderHeaders,
    explicit: crate::types::ProviderHeaders,
) -> crate::types::ProviderHeaders {
    let mut headers: crate::types::ProviderHeaders = resolved.into_iter().collect();
    headers.extend(explicit);
    headers
}

fn merge_env(
    resolved: AuthProviderEnv,
    explicit: crate::types::ProviderEnv,
) -> crate::types::ProviderEnv {
    let mut env: crate::types::ProviderEnv = resolved.into_iter().collect();
    env.extend(explicit);
    env
}

fn error_images(model: &ImagesModel, message: String) -> AssistantImages {
    AssistantImages {
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        output: Vec::new(),
        response_id: None,
        usage: None,
        stop_reason: ImagesStopReason::Error,
        error_message: Some(message),
        timestamp: unix_timestamp_ms(),
    }
}

fn unix_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn default_file_exists(path: &str) -> bool {
    let resolved = if let Some(rest) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(|home| std::path::PathBuf::from(home).join(rest))
            .unwrap_or_else(|| std::path::PathBuf::from(path))
    } else {
        std::path::PathBuf::from(path)
    };

    resolved.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, id: &str) -> ImagesModel {
        ImagesModel {
            id: id.to_string(),
            name: id.to_string(),
            api: "api".to_string(),
            provider: provider.to_string(),
            base_url: String::new(),
            input: vec![crate::types::ModelInput::Text],
            output: vec![crate::types::ModelOutput::Text],
            cost: crate::types::ModelCost::default(),
            headers: None,
        }
    }

    #[test]
    fn provider_collection_finds_models() {
        let mut models = create_images_models();
        models.set_provider(create_images_provider(CreateImagesProviderOptions {
            id: "openrouter".into(),
            name: None,
            auth: ProviderAuth::default(),
            models: vec![model("openrouter", "m1")],
            refresh_models: None,
            generate_images: Arc::new(|model, _, _| {
                Box::pin(async move {
                    AssistantImages {
                        api: model.api,
                        provider: model.provider,
                        model: model.id,
                        output: vec![crate::types::ToolResultContentBlock::Text(
                            crate::types::TextContent {
                                content_type: crate::types::TextContentType::Text,
                                text: "ok".into(),
                                text_signature: None,
                            },
                        )],
                        response_id: Some("response".into()),
                        usage: None,
                        stop_reason: ImagesStopReason::Stop,
                        error_message: None,
                        timestamp: 1,
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

        assert_eq!(result.stop_reason, ImagesStopReason::Error);
        assert_eq!(
            result.error_message,
            Some("Unknown provider: missing".into())
        );
    }
}
