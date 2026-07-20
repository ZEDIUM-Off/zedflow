//! Runtime chat model collection ported from Pi's `packages/ai/src/models.ts`.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::future::{BoxFuture, FutureExt, Shared, join_all};

use crate::auth::resolve::{
    AuthResolutionOverrides, ModelsError, ModelsErrorCode, resolve_provider_auth,
};
use crate::auth::types::{
    AuthContext, AuthFuture, AuthModel, AuthProvider, CredentialStore,
    ProviderEnv as AuthProviderEnv, ProviderHeaders as AuthProviderHeaders, ResolvedAuth,
};
use crate::types::{
    AssistantContentBlock, AssistantMessageEvent, AssistantMessageRole, ErrorStopReason,
    ProviderHeaders, ProviderStreams, StopReason, TextContent, TextContentType, Usage, UsageCost,
};

pub use crate::auth::types::ProviderAuth;
pub use crate::types::{
    Api, AssistantMessage, AssistantMessageEventStream, Context, Model, SimpleStreamOptions,
    StreamOptions,
};

/// Dynamic model refresh callback.
pub type RefreshModelSource =
    Arc<dyn Fn() -> BoxFuture<'static, Result<Vec<Model>, ModelsError>> + Send + Sync>;

type SharedRefresh = Shared<BoxFuture<'static, Result<(), ModelsError>>>;

/// Last-known model source for a provider.
pub type ModelSource = Arc<Mutex<Result<Vec<Model>, ModelsError>>>;

/// Single implementation or dispatch map keyed by `model.api`.
#[derive(Clone)]
pub enum ProviderApi {
    /// One stream implementation handles every model.
    Single(ProviderStreams),
    /// Stream implementations are selected by `model.api`.
    ByApi(HashMap<Api, ProviderStreams>),
}

impl ProviderApi {
    fn streams_for(&self, api: &str) -> Option<&ProviderStreams> {
        match self {
            Self::Single(streams) => Some(streams),
            Self::ByApi(streams) => streams.get(api),
        }
    }
}

impl fmt::Debug for ProviderApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(_) => f.write_str("ProviderApi::Single(..)"),
            Self::ByApi(streams) => f
                .debug_struct("ProviderApi::ByApi")
                .field("apis", &streams.keys().collect::<Vec<_>>())
                .finish(),
        }
    }
}

/// Provider runtime unit.
#[derive(Clone)]
pub struct Provider {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Provider-level base URL metadata.
    pub base_url: Option<String>,
    /// Provider-level HTTP headers metadata.
    pub headers: Option<ProviderHeaders>,
    /// Provider auth metadata.
    pub auth: ProviderAuth,
    /// Last-known models.
    pub model_source: ModelSource,
    /// Optional dynamic refresh source.
    pub refresh_source: Option<RefreshModelSource>,
    in_flight_refresh: Arc<Mutex<Option<SharedRefresh>>>,
    /// Single stream implementation or per-API dispatch map.
    pub api: ProviderApi,
}

impl fmt::Debug for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("headers", &self.headers)
            .field(
                "auth_configured",
                &(self.auth.api_key.is_some() || self.auth.oauth.is_some()),
            )
            .field("model_source", &self.model_source)
            .field("refresh_source", &self.refresh_source.is_some())
            .field("api", &self.api)
            .finish()
    }
}

impl Provider {
    /// Returns last-known provider models, or an empty list if the source failed.
    #[must_use]
    pub fn get_models(&self) -> Vec<Model> {
        self.get_models_result().unwrap_or_default()
    }

    /// Returns the fallible last-known provider model source.
    pub fn get_models_result(&self) -> Result<Vec<Model>, ModelsError> {
        self.model_source
            .lock()
            .expect("models lock poisoned")
            .clone()
    }

    /// Refreshes dynamic models if configured. Concurrent calls share one fetch.
    pub async fn refresh_models(&self) -> Result<(), ModelsError> {
        let Some(refresh) = &self.refresh_source else {
            return Ok(());
        };

        let in_flight = {
            let mut guard = self
                .in_flight_refresh
                .lock()
                .expect("refresh lock poisoned");
            if let Some(in_flight) = guard.clone() {
                in_flight
            } else {
                let refresh = Arc::clone(refresh);
                let model_source = Arc::clone(&self.model_source);
                let in_flight_refresh = Arc::clone(&self.in_flight_refresh);
                let in_flight = async move {
                    let result = refresh().await;
                    if let Ok(models) = result.as_ref() {
                        *model_source.lock().expect("models lock poisoned") = Ok(models.clone());
                    }
                    *in_flight_refresh.lock().expect("refresh lock poisoned") = None;
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

    /// Opens a message event stream.
    #[must_use]
    pub fn stream(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        self.api.streams_for(&model.api).map_or_else(
            || stream_error(missing_api_message(self, model)),
            |streams| (streams.stream)(model, context, options),
        )
    }

    /// Opens a simple message event stream.
    #[must_use]
    pub fn stream_simple(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&SimpleStreamOptions>,
    ) -> AssistantMessageEventStream {
        self.api.streams_for(&model.api).map_or_else(
            || stream_error(missing_api_message(self, model)),
            |streams| (streams.stream_simple)(model, context, options),
        )
    }
}

/// Mutable runtime collection of providers.
pub struct Models {
    providers: Vec<Provider>,
    auth_context: Arc<dyn AuthContext>,
    credentials: Arc<dyn CredentialStore>,
}

impl Default for Models {
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

impl Models {
    /// Upsert/replace by provider id without moving its insertion slot.
    pub fn set_provider(&mut self, provider: Provider) {
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

    /// Delete provider by id.
    pub fn delete_provider(&mut self, id: &str) {
        self.providers.retain(|provider| provider.id != id);
    }

    /// Clear all providers.
    pub fn clear_providers(&mut self) {
        self.providers.clear();
    }

    /// Returns all providers in insertion order.
    #[must_use]
    pub fn get_providers(&self) -> Vec<Provider> {
        self.providers.clone()
    }

    /// Returns one provider.
    #[must_use]
    pub fn get_provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    /// Returns models from one provider or all providers in insertion order.
    #[must_use]
    pub fn get_models(&self, provider: Option<&str>) -> Vec<Model> {
        if let Some(provider) = provider {
            return self
                .get_provider(provider)
                .map_or_else(Vec::new, Provider::get_models);
        }
        self.providers
            .iter()
            .flat_map(Provider::get_models)
            .collect()
    }

    /// Looks up a model by provider and id.
    #[must_use]
    pub fn get_model(&self, provider: &str, id: &str) -> Option<Model> {
        self.get_models(Some(provider))
            .into_iter()
            .find(|model| model.id == id)
    }

    /// Refreshes one provider, or all providers best-effort.
    pub async fn refresh(&self, provider: Option<&str>) -> Result<(), ModelsError> {
        if let Some(provider) = provider {
            let Some(entry) = self.get_provider(provider) else {
                return Ok(());
            };
            return entry.refresh_models().await.map_err(|error| {
                if error.code() == ModelsErrorCode::ModelSource {
                    error
                } else {
                    ModelsError::new(
                        ModelsErrorCode::ModelSource,
                        format!("Model refresh failed for {provider}"),
                    )
                }
            });
        }
        let _ = join_all(self.providers.iter().map(Provider::refresh_models)).await;
        Ok(())
    }

    /// Resolves stored or ambient provider auth for a model.
    pub async fn get_auth(&self, model: &Model) -> Result<Option<ResolvedAuth>, ModelsError> {
        let Some(provider) = self.get_provider(&model.provider) else {
            return Ok(None);
        };
        self.resolve_auth(provider, model, None).await
    }

    async fn resolve_auth(
        &self,
        provider: &Provider,
        model: &Model,
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
    }

    /// Opens a stream through the owning provider without blocking on auth.
    #[must_use]
    pub fn stream(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        let outer = AssistantMessageEventStream::new();
        let worker_stream = outer.clone();
        let provider = self.get_provider(&model.provider).cloned();
        let model = model.clone();
        let context = context.clone();
        let options = options.cloned();
        let credentials = Arc::clone(&self.credentials);
        let auth_context = Arc::clone(&self.auth_context);
        let identity = crate::utils::runtime::StreamIdentity::new(
            model.api.clone(),
            model.provider.clone(),
            model.id.clone(),
        );
        crate::utils::runtime::spawn_stream_worker(outer.clone(), identity, async move {
            let Some(provider) = provider else {
                worker_stream.push(AssistantMessageEvent::Error {
                    reason: ErrorStopReason::Error,
                    error: unknown_provider_message(&model, &model.provider),
                });
                return;
            };
            let overrides = auth_overrides(options.as_ref());
            let resolution = resolve_provider_auth(
                &AuthProvider {
                    id: provider.id.clone(),
                    auth: provider.auth.clone(),
                },
                &auth_model(&model),
                credentials.as_ref(),
                auth_context.as_ref(),
                overrides.as_ref(),
            )
            .await;
            let (request_model, request_options) = match resolution {
                Ok(resolution) => merge_resolved_auth(&model, options, resolution),
                Err(error) => {
                    worker_stream.push(AssistantMessageEvent::Error {
                        reason: ErrorStopReason::Error,
                        error: auth_stream_error_message(&model, error),
                    });
                    return;
                }
            };
            let mut inner = provider.stream(&request_model, &context, request_options.as_ref());
            while let Some(event) = inner.next().await {
                worker_stream.push(event);
            }
        });
        outer
    }

    /// Opens a simple stream through the owning provider without blocking on auth.
    #[must_use]
    pub fn stream_simple(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&SimpleStreamOptions>,
    ) -> AssistantMessageEventStream {
        let outer = AssistantMessageEventStream::new();
        let worker_stream = outer.clone();
        let provider = self.get_provider(&model.provider).cloned();
        let model = model.clone();
        let context = context.clone();
        let options = options.cloned();
        let credentials = Arc::clone(&self.credentials);
        let auth_context = Arc::clone(&self.auth_context);
        let identity = crate::utils::runtime::StreamIdentity::new(
            model.api.clone(),
            model.provider.clone(),
            model.id.clone(),
        );
        crate::utils::runtime::spawn_stream_worker(outer.clone(), identity, async move {
            let Some(provider) = provider else {
                worker_stream.push(AssistantMessageEvent::Error {
                    reason: ErrorStopReason::Error,
                    error: unknown_provider_message(&model, &model.provider),
                });
                return;
            };
            let stream_options = options.as_ref().map(|options| &options.stream);
            let overrides = auth_overrides(stream_options);
            let resolution = resolve_provider_auth(
                &AuthProvider {
                    id: provider.id.clone(),
                    auth: provider.auth.clone(),
                },
                &auth_model(&model),
                credentials.as_ref(),
                auth_context.as_ref(),
                overrides.as_ref(),
            )
            .await;
            let (request_model, request_stream_options) = match resolution {
                Ok(resolution) => merge_resolved_auth(&model, stream_options.cloned(), resolution),
                Err(error) => {
                    worker_stream.push(AssistantMessageEvent::Error {
                        reason: ErrorStopReason::Error,
                        error: auth_stream_error_message(&model, error),
                    });
                    return;
                }
            };
            let request_options = match (options, request_stream_options) {
                (Some(mut options), Some(stream)) => {
                    options.stream = stream;
                    Some(options)
                }
                (Some(options), None) => Some(options),
                (None, Some(stream)) => Some(SimpleStreamOptions {
                    stream,
                    ..SimpleStreamOptions::default()
                }),
                (None, None) => None,
            };
            let mut inner =
                provider.stream_simple(&request_model, &context, request_options.as_ref());
            while let Some(event) = inner.next().await {
                worker_stream.push(event);
            }
        });
        outer
    }

    /// Collects the stream into a single assistant message.
    pub async fn complete(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessage {
        self.stream(model, context, options).result().await
    }

    /// Collects the simple stream into a single assistant message.
    pub async fn complete_simple(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&SimpleStreamOptions>,
    ) -> AssistantMessage {
        self.stream_simple(model, context, options).result().await
    }
}

/// Creates an empty provider collection.
#[must_use]
pub fn create_models() -> Models {
    Models::default()
}

/// Creates an empty provider collection with a custom auth context.
#[must_use]
pub fn create_models_with_auth_context(context: impl AuthContext + 'static) -> Models {
    create_models_with_auth_context_and_credentials(
        context,
        crate::auth::credential_store::InMemoryCredentialStore::new(),
    )
}

/// Creates an empty provider collection with a custom credential store.
#[must_use]
pub fn create_models_with_credentials(credentials: impl CredentialStore + 'static) -> Models {
    Models {
        providers: Vec::new(),
        auth_context: Arc::new(DefaultAuthContext),
        credentials: Arc::new(credentials),
    }
}

/// Creates an empty provider collection with custom auth context and credentials.
#[must_use]
pub fn create_models_with_auth_context_and_credentials(
    context: impl AuthContext + 'static,
    credentials: impl CredentialStore + 'static,
) -> Models {
    Models {
        providers: Vec::new(),
        auth_context: Arc::new(context),
        credentials: Arc::new(credentials),
    }
}

fn auth_model(model: &Model) -> AuthModel {
    AuthModel {
        provider: model.provider.clone(),
        api: model.api.clone(),
        id: model.id.clone(),
        base_url: non_empty(model.base_url.clone()),
    }
}

fn auth_overrides(options: Option<&StreamOptions>) -> Option<AuthResolutionOverrides> {
    let options = options?;
    (options.api_key.is_some() || options.env.is_some()).then(|| AuthResolutionOverrides {
        api_key: options.api_key.clone(),
        env: options.env.as_ref().map(|env| {
            env.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        }),
    })
}

fn merge_resolved_auth(
    model: &Model,
    options: Option<StreamOptions>,
    resolution: Option<ResolvedAuth>,
) -> (Model, Option<StreamOptions>) {
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
    request_options.headers = merge_headers(resolution.auth.headers, request_options.headers);
    request_options.env = merge_env(resolution.env, request_options.env);

    (request_model, Some(request_options))
}

fn merge_headers(
    resolved: Option<AuthProviderHeaders>,
    explicit: Option<ProviderHeaders>,
) -> Option<ProviderHeaders> {
    if resolved.is_none() && explicit.is_none() {
        return None;
    }
    let mut headers = resolved
        .unwrap_or_default()
        .into_iter()
        .collect::<ProviderHeaders>();
    headers.extend(explicit.unwrap_or_default());
    Some(headers)
}

fn merge_env(
    resolved: Option<AuthProviderEnv>,
    explicit: Option<crate::types::ProviderEnv>,
) -> Option<crate::types::ProviderEnv> {
    if resolved.is_none() && explicit.is_none() {
        return None;
    }
    let mut env = resolved
        .unwrap_or_default()
        .into_iter()
        .collect::<crate::types::ProviderEnv>();
    env.extend(explicit.unwrap_or_default());
    Some(env)
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
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

fn auth_stream_error_message(model: &Model, error: ModelsError) -> AssistantMessage {
    let message = error.to_string();
    assistant_message(model, StopReason::Error, message.clone(), Some(message))
}

/// Converts a setup failure into the one terminal event a request stream can expose.
pub(crate) fn terminal_stream_error(
    model: &Model,
    message: impl Into<String>,
) -> AssistantMessageEventStream {
    let message = message.into();
    stream_error(assistant_message(
        model,
        StopReason::Error,
        message.clone(),
        Some(message),
    ))
}

fn unknown_provider_message(model: &Model, provider: &str) -> AssistantMessage {
    assistant_message(
        model,
        StopReason::Error,
        format!("Unknown provider: {provider}"),
        Some(format!("Unknown provider: {provider}")),
    )
}

fn missing_api_message(provider: &Provider, model: &Model) -> AssistantMessage {
    let message = format!(
        "Provider {} has no API implementation for \"{}\"",
        provider.id, model.api
    );
    assistant_message(model, StopReason::Error, message.clone(), Some(message))
}

fn stream_error(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Error,
        error: message,
    });
    stream
}

#[cfg(test)]
fn stream_done(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: crate::types::DoneStopReason::Stop,
        message,
    });
    stream
}

fn assistant_message(
    model: &Model,
    stop_reason: StopReason,
    text: String,
    error_message: Option<String>,
) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: if text.is_empty() {
            Vec::new()
        } else {
            vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text,
                text_signature: None,
            })]
        },
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            cost: UsageCost::default(),
            ..Usage::default()
        },
        stop_reason,
        error_message,
        timestamp: 0,
    }
}

/// Options for [`create_provider`].
pub struct CreateProviderOptions {
    /// Provider id.
    pub id: String,
    /// Optional display name.
    pub name: Option<String>,
    /// Provider-level base URL metadata.
    pub base_url: Option<String>,
    /// Provider-level headers metadata.
    pub headers: Option<ProviderHeaders>,
    /// Provider auth metadata.
    pub auth: ProviderAuth,
    /// Initial models.
    pub models: Vec<Model>,
    /// Dynamic refresh callback.
    pub refresh_models: Option<RefreshModelSource>,
    /// Single implementation or per-API dispatch map.
    pub api: ProviderApi,
}

/// Builds a provider from parts.
#[must_use]
pub fn create_provider(input: CreateProviderOptions) -> Provider {
    Provider {
        id: input.id.clone(),
        name: input.name.unwrap_or(input.id),
        base_url: input.base_url,
        headers: input.headers,
        auth: input.auth,
        model_source: Arc::new(Mutex::new(Ok(input.models))),
        refresh_source: input.refresh_models,
        in_flight_refresh: Arc::new(Mutex::new(None)),
        api: input.api,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn model_collection_registers_provider() {
        let provider = create_provider(CreateProviderOptions {
            id: "p".into(),
            name: None,
            models: vec![Model {
                provider: "p".into(),
                id: "m".into(),
                api: "a".into(),
                ..Model::default()
            }],
            base_url: None,
            headers: None,
            auth: ProviderAuth::default(),
            refresh_models: None,
            api: ProviderApi::Single(ProviderStreams {
                stream: Arc::new(|model, _, _| {
                    stream_done(assistant_message(
                        model,
                        StopReason::Stop,
                        "ok".into(),
                        None,
                    ))
                }),
                stream_simple: Arc::new(|model, _, _| {
                    stream_done(assistant_message(
                        model,
                        StopReason::Stop,
                        "ok".into(),
                        None,
                    ))
                }),
            }),
        });
        let mut models = create_models();
        models.set_provider(provider);
        assert_eq!(models.get_model("p", "m").expect("model").api, "a");
        assert_eq!(
            models
                .complete(
                    &Model {
                        provider: "p".into(),
                        id: "m".into(),
                        api: "a".into(),
                        ..Model::default()
                    },
                    &Context::default(),
                    None
                )
                .await
                .content,
            vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "ok".into(),
                text_signature: None,
            })]
        );
    }
}
