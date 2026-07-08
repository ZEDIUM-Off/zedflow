//! Image API provider registry ported from Pi's `packages/ai/src/images-api-registry.ts`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

/// Image API identifier, such as `openrouter-images`.
pub type ImagesApi = String;

/// Minimal image model shape accepted by registry callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagesModel {
    /// API implementation identifier for this model.
    pub api: ImagesApi,
    /// Provider model identifier.
    pub id: String,
}

/// Minimal image request context accepted by registry callbacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesContext {
    /// Text prompt or serialized image content payload.
    pub input: Vec<String>,
}

/// Minimal image request options accepted by registry callbacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImagesOptions {
    /// Optional API key supplied by the caller.
    pub api_key: Option<String>,
}

/// Assistant image response returned by registry callbacks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssistantImages {
    /// Generated image payloads or data URLs.
    pub images: Vec<String>,
}

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

/// Function signature used by registered image providers.
pub type ImagesApiFunction = dyn Fn(&ImagesModel, &ImagesContext, Option<&ImagesOptions>) -> ImagesResult<AssistantImages>
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

static IMAGES_API_PROVIDER_REGISTRY: LazyLock<
    RwLock<HashMap<ImagesApi, RegisteredImagesApiProvider>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

fn wrap_generate_images(
    api: ImagesApi,
    generate_images: Arc<ImagesApiFunction>,
) -> Arc<ImagesApiFunction> {
    Arc::new(move |model, context, options| {
        if model.api != api {
            return Err(ImagesApiRegistryError::MismatchedApi {
                actual: model.api.clone(),
                expected: api.clone(),
            });
        }
        generate_images(model, context, options)
    })
}

/// Register or replace an image API provider.
///
/// This matches Pi's map semantics: registering the same `api` overwrites the previous provider.
pub fn register_images_api_provider(provider: ImagesApiProvider, source_id: Option<String>) {
    let wrapped = ImagesApiProviderInternal {
        api: provider.api.clone(),
        generate_images: wrap_generate_images(provider.api.clone(), provider.generate_images),
    };
    let registered = RegisteredImagesApiProvider {
        provider: wrapped,
        source_id,
    };
    IMAGES_API_PROVIDER_REGISTRY
        .write()
        .expect("images API registry lock poisoned")
        .insert(provider.api, registered);
}

/// Get a registered image API provider by API identifier.
#[must_use]
pub fn get_images_api_provider(api: &str) -> Option<ImagesApiProviderInternal> {
    IMAGES_API_PROVIDER_REGISTRY
        .read()
        .expect("images API registry lock poisoned")
        .get(api)
        .map(|registered| registered.provider.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get_provider() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "test-images".to_string(),
                generate_images: Arc::new(|_, _, _| {
                    Ok(AssistantImages {
                        images: vec!["ok".into()],
                    })
                }),
            },
            Some("test".to_string()),
        );

        let provider = get_images_api_provider("test-images").expect("provider registered");
        let images = (provider.generate_images)(
            &ImagesModel {
                api: "test-images".into(),
                id: "model".into(),
            },
            &ImagesContext::default(),
            None,
        )
        .expect("images generated");

        assert_eq!(images.images, ["ok"]);
    }

    #[test]
    fn wrapped_provider_rejects_mismatched_api() {
        register_images_api_provider(
            ImagesApiProvider {
                api: "expected".to_string(),
                generate_images: Arc::new(|_, _, _| Ok(AssistantImages::default())),
            },
            None,
        );

        let provider = get_images_api_provider("expected").expect("provider registered");
        let error = (provider.generate_images)(
            &ImagesModel {
                api: "actual".into(),
                id: "model".into(),
            },
            &ImagesContext::default(),
            None,
        )
        .expect_err("mismatched API should fail");

        assert_eq!(
            error.to_string(),
            "Mismatched api: actual expected expected"
        );
    }
}
