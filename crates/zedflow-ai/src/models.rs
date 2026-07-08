//! Runtime chat model collection ported from Pi's `packages/ai/src/models.ts`.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::auth::resolve::{AuthResult, ModelsError, ModelsErrorCode};

/// API identifier for a model stream implementation.
pub type Api = String;

/// Minimal chat model shape used by the collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Provider id that owns this model.
    pub provider: String,
    /// Model id.
    pub id: String,
    /// API implementation id.
    pub api: Api,
}

/// Minimal stream options placeholder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamOptions {
    /// Optional API key supplied by caller.
    pub api_key: Option<String>,
}

/// Minimal assistant message returned by `complete`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssistantMessage {
    /// Text content.
    pub text: String,
}

/// Minimal stream event list. This is intentionally small until concrete stream rows own event types.
pub type AssistantMessageEventStream = Vec<AssistantMessage>;

/// Provider runtime unit.
#[derive(Clone)]
pub struct Provider {
    /// Provider id.
    pub id: String,
    /// Display name.
    pub name: String,
    models: Arc<Mutex<Vec<Model>>>,
    refresh_models: Option<Arc<dyn Fn() -> Result<Vec<Model>, ModelsError> + Send + Sync>>,
    stream:
        Arc<dyn Fn(&Model, Option<&StreamOptions>) -> AssistantMessageEventStream + Send + Sync>,
}

impl fmt::Debug for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

impl Provider {
    /// Returns last-known provider models.
    #[must_use]
    pub fn get_models(&self) -> Vec<Model> {
        self.models.lock().expect("models lock poisoned").clone()
    }

    /// Refreshes dynamic models if configured.
    pub fn refresh_models(&self) -> Result<(), ModelsError> {
        let Some(refresh) = &self.refresh_models else {
            return Ok(());
        };
        let models = refresh()?;
        *self.models.lock().expect("models lock poisoned") = models;
        Ok(())
    }

    /// Opens a message event stream.
    #[must_use]
    pub fn stream(
        &self,
        model: &Model,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        (self.stream)(model, options)
    }
}

/// Mutable runtime collection of providers.
#[derive(Default)]
pub struct Models {
    providers: HashMap<String, Provider>,
}

impl Models {
    /// Upsert/replace by provider id.
    pub fn set_provider(&mut self, provider: Provider) {
        self.providers.insert(provider.id.clone(), provider);
    }

    /// Delete provider by id.
    pub fn delete_provider(&mut self, id: &str) {
        self.providers.remove(id);
    }

    /// Clear all providers.
    pub fn clear_providers(&mut self) {
        self.providers.clear();
    }

    /// Returns all providers.
    #[must_use]
    pub fn get_providers(&self) -> Vec<Provider> {
        self.providers.values().cloned().collect()
    }

    /// Returns one provider.
    #[must_use]
    pub fn get_provider(&self, id: &str) -> Option<&Provider> {
        self.providers.get(id)
    }

    /// Returns models from one provider or all providers.
    #[must_use]
    pub fn get_models(&self, provider: Option<&str>) -> Vec<Model> {
        if let Some(provider) = provider {
            return self
                .providers
                .get(provider)
                .map_or_else(Vec::new, Provider::get_models);
        }
        self.providers
            .values()
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
    pub fn refresh(&self, provider: Option<&str>) -> Result<(), ModelsError> {
        if let Some(provider) = provider {
            let Some(entry) = self.providers.get(provider) else {
                return Ok(());
            };
            return entry.refresh_models().map_err(|error| {
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
        for entry in self.providers.values() {
            let _ = entry.refresh_models();
        }
        Ok(())
    }

    /// Resolves auth for a model. Full key-store/OAuth wiring stays in auth rows.
    #[must_use]
    pub fn get_auth(&self, model: &Model) -> Option<AuthResult> {
        self.providers
            .get(&model.provider)
            .map(|_| AuthResult::default())
    }

    /// Opens a stream through the owning provider.
    #[must_use]
    pub fn stream(
        &self,
        model: &Model,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        self.providers
            .get(&model.provider)
            .map_or_else(Vec::new, |provider| provider.stream(model, options))
    }

    /// Collects the stream into a single assistant message.
    #[must_use]
    pub fn complete(&self, model: &Model, options: Option<&StreamOptions>) -> AssistantMessage {
        self.stream(model, options)
            .into_iter()
            .last()
            .unwrap_or_default()
    }
}

/// Creates an empty provider collection.
#[must_use]
pub fn create_models() -> Models {
    Models::default()
}

/// Options for [`create_provider`].
pub struct CreateProviderOptions {
    /// Provider id.
    pub id: String,
    /// Optional display name.
    pub name: Option<String>,
    /// Initial models.
    pub models: Vec<Model>,
    /// Dynamic refresh callback.
    pub refresh_models: Option<Arc<dyn Fn() -> Result<Vec<Model>, ModelsError> + Send + Sync>>,
    /// Stream callback.
    pub stream:
        Arc<dyn Fn(&Model, Option<&StreamOptions>) -> AssistantMessageEventStream + Send + Sync>,
}

/// Builds a provider from parts.
#[must_use]
pub fn create_provider(input: CreateProviderOptions) -> Provider {
    Provider {
        id: input.id.clone(),
        name: input.name.unwrap_or(input.id),
        models: Arc::new(Mutex::new(input.models)),
        refresh_models: input.refresh_models,
        stream: input.stream,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_collection_registers_provider() {
        let provider = create_provider(CreateProviderOptions {
            id: "p".into(),
            name: None,
            models: vec![Model {
                provider: "p".into(),
                id: "m".into(),
                api: "a".into(),
            }],
            refresh_models: None,
            stream: Arc::new(|_, _| vec![AssistantMessage { text: "ok".into() }]),
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
                        api: "a".into()
                    },
                    None
                )
                .text,
            "ok"
        );
    }
}
