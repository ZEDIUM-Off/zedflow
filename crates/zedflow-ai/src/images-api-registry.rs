//! Image API provider registry ported from Pi's `packages/ai/src/images-api-registry.ts`.

use std::sync::{Arc, LazyLock, RwLock};

use futures::future::BoxFuture;

pub use crate::api::openrouter_images::{
    AssistantImages, ImagesContext, ImagesModel, ImagesOptions,
};

/// Image API identifier, such as `openrouter-images`.
pub type ImagesApi = String;

/// Result type for image registry callbacks.
pub type ImagesResult<T> = Result<T, ImagesApiRegistryError>;

/// Errors returned by the image API registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImagesApiRegistryError {
    /// The model API does not match the provider API.
    MismatchedApi {
        /// API found on the model.
        actual: ImagesApi,
        /// API expected by the provider.
        expected: ImagesApi,
    },
}

impl std::fmt::Display for ImagesApiRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MismatchedApi { actual, expected } => {
                write!(f, "Mismatched api: {actual} expected {expected}")
            }
        }
    }
}

impl std::error::Error for ImagesApiRegistryError {}

/// Future returned by image API callbacks.
pub type ImagesApiFuture = BoxFuture<'static, ImagesResult<AssistantImages>>;

/// Function signature used by registered image providers.
pub type ImagesApiFunction = dyn Fn(ImagesModel, ImagesContext, Option<ImagesOptions>) -> ImagesApiFuture
    + Send
    + Sync
    + 'static;

/// Public image API provider shape.
#[derive(Clone)]
pub struct ImagesApiProvider {
    /// API identifier served by this provider.
    pub api: ImagesApi,
    /// Image generation callback.
    pub generate_images: Arc<ImagesApiFunction>,
}

/// Internal registered provider shape after API mismatch checks are wrapped in.
#[derive(Clone)]
pub struct ImagesApiProviderInternal {
    /// API identifier served by this provider.
    pub api: ImagesApi,
    /// Wrapped image generation callback.
    pub generate_images: Arc<ImagesApiFunction>,
}

#[derive(Clone)]
struct RegisteredImagesApiProvider {
    provider: ImagesApiProviderInternal,
    #[allow(dead_code)]
    source_id: Option<String>,
}

// Vec deliberately mirrors JavaScript Map insertion order. Replacing an API keeps its slot.
static IMAGES_API_PROVIDER_REGISTRY: LazyLock<RwLock<Vec<RegisteredImagesApiProvider>>> =
    LazyLock::new(|| RwLock::new(Vec::new()));

fn wrap_generate_images(
    api: ImagesApi,
    generate_images: Arc<ImagesApiFunction>,
) -> Arc<ImagesApiFunction> {
    Arc::new(move |model, context, options| {
        if model.api != api {
            let actual = model.api;
            let expected = api.clone();
            return Box::pin(async move {
                Err(ImagesApiRegistryError::MismatchedApi { actual, expected })
            });
        }
        generate_images(model, context, options)
    })
}

/// Register or replace an image API provider.
///
/// Like Pi's `Map.set`, replacement preserves the provider's insertion slot.
pub fn register_images_api_provider(provider: ImagesApiProvider, source_id: Option<String>) {
    let api = provider.api.clone();
    let registered = RegisteredImagesApiProvider {
        provider: ImagesApiProviderInternal {
            api: api.clone(),
            generate_images: wrap_generate_images(api.clone(), provider.generate_images),
        },
        source_id,
    };
    let mut registry = IMAGES_API_PROVIDER_REGISTRY
        .write()
        .expect("images API registry lock poisoned");
    if let Some(existing) = registry.iter_mut().find(|entry| entry.provider.api == api) {
        *existing = registered;
    } else {
        registry.push(registered);
    }
}

/// Get a registered image API provider by API identifier.
#[must_use]
pub fn get_images_api_provider(api: &str) -> Option<ImagesApiProviderInternal> {
    IMAGES_API_PROVIDER_REGISTRY
        .read()
        .expect("images API registry lock poisoned")
        .iter()
        .find(|registered| registered.provider.api == api)
        .map(|registered| registered.provider.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(api: &str) -> ImagesModel {
        ImagesModel {
            id: "model".into(),
            api: api.into(),
            provider: "test".into(),
            base_url: String::new(),
            headers: Default::default(),
            output: Vec::new(),
            cost: Default::default(),
        }
    }

    #[test]
    fn register_and_get_provider() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "test-images".to_string(),
                generate_images: Arc::new(|model, _, _| {
                    Box::pin(async move {
                        Ok(AssistantImages {
                            api: model.api,
                            provider: model.provider,
                            model: model.id,
                            output: Vec::new(),
                            response_id: None,
                            usage: None,
                            stop_reason: crate::api::openrouter_images::ImagesStopReason::Stop,
                            error_message: None,
                            timestamp: 1,
                        })
                    })
                }),
            },
            Some("test".to_string()),
        );

        let provider = get_images_api_provider("test-images").expect("provider registered");
        let images = futures::executor::block_on((provider.generate_images)(
            model("test-images"),
            ImagesContext::default(),
            None,
        ))
        .expect("images generated");

        assert_eq!(
            images.stop_reason,
            crate::api::openrouter_images::ImagesStopReason::Stop
        );
    }

    #[test]
    fn wrapped_provider_rejects_mismatched_api() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "expected".to_string(),
                generate_images: Arc::new(|_, _, _| unreachable!()),
            },
            None,
        );

        let provider = get_images_api_provider("expected").expect("provider registered");
        let error = futures::executor::block_on((provider.generate_images)(
            model("actual"),
            ImagesContext::default(),
            None,
        ))
        .expect_err("mismatched API should fail");

        assert_eq!(
            error.to_string(),
            "Mismatched api: actual expected expected"
        );
    }
}
