//! OpenRouter image provider factory ported from Pi's `packages/ai/src/providers/openrouter-images.ts`.

use std::sync::Arc;

use zedflow_core::error::Result;

use crate::api::openrouter_images as api;
use crate::image_models::{KnownImagesProvider, get_image_model, get_image_models};
use crate::image_models_generated::ImageModelContent;
use crate::images_models::{
    AssistantImages, CreateImagesProviderOptions, ImagesContext, ImagesModel, ImagesOptions,
    ImagesProvider, ProviderAuth, create_images_provider,
};

/// OpenRouter image provider id used by Pi.
pub const OPENROUTER_IMAGES_PROVIDER_ID: &str = "openrouter";

/// OpenRouter image provider display name used by Pi.
pub const OPENROUTER_IMAGES_PROVIDER_NAME: &str = "OpenRouter";

/// OpenRouter Images API id used by Pi image models.
pub const OPENROUTER_IMAGES_API: &str = "openrouter-images";

/// OpenRouter API-key auth prompt label used by Pi.
pub const OPENROUTER_IMAGES_API_KEY_AUTH_NAME: &str = "OpenRouter API key";

/// Environment variables checked for OpenRouter image API-key auth, in Pi precedence order.
pub const OPENROUTER_IMAGES_API_KEY_ENV_VARS: &[&str] = &["OPENROUTER_API_KEY"];

/// Creates Pi's OpenRouter image provider from the static image catalog.
pub fn openrouter_images_provider() -> Result<ImagesProvider> {
    Ok(create_images_provider(CreateImagesProviderOptions {
        id: OPENROUTER_IMAGES_PROVIDER_ID.into(),
        name: Some(OPENROUTER_IMAGES_PROVIDER_NAME.into()),
        auth: ProviderAuth {
            api_key: Some(Arc::new(EnvApiKeyAuth {
                name: OPENROUTER_IMAGES_API_KEY_AUTH_NAME.to_owned(),
                env_vars: OPENROUTER_IMAGES_API_KEY_ENV_VARS,
            })),
            oauth: None,
        },
        models: get_image_models(KnownImagesProvider::Openrouter)
            .into_iter()
            .map(|model| ImagesModel {
                api: model.api.to_string(),
                id: model.id.to_string(),
                provider: OPENROUTER_IMAGES_PROVIDER_ID.into(),
                base_url: Some(model.base_url.to_string()),
            })
            .collect(),
        refresh_models: None,
        generate_images: Arc::new(|model, context, options| {
            Box::pin(async move { generate_openrouter_images(model, context, options).await })
        }),
    }))
}

#[derive(Debug)]
struct EnvApiKeyAuth {
    name: String,
    env_vars: &'static [&'static str],
}

impl crate::auth::types::ApiKeyAuth for EnvApiKeyAuth {
    fn name(&self) -> &str {
        &self.name
    }

    fn resolve<'a>(
        &'a self,
        input: crate::auth::types::ApiKeyResolveInput<'a>,
    ) -> crate::auth::types::AuthFuture<
        'a,
        crate::auth::types::AuthResult<Option<crate::auth::types::ResolvedAuth>>,
    > {
        Box::pin(async move {
            if let Some(key) = input
                .credential
                .and_then(|credential| credential.key.as_deref())
                .filter(|key| !key.is_empty())
            {
                return Ok(Some(crate::auth::types::ResolvedAuth {
                    auth: crate::auth::types::ModelAuth {
                        api_key: Some(key.to_owned()),
                        ..crate::auth::types::ModelAuth::default()
                    },
                    env: input
                        .credential
                        .and_then(|credential| credential.env.clone()),
                    source: Some("stored credential".to_owned()),
                }));
            }

            for name in self.env_vars {
                if let Some(value) = input.ctx.env(name).await.filter(|value| !value.is_empty()) {
                    return Ok(Some(crate::auth::types::ResolvedAuth {
                        auth: crate::auth::types::ModelAuth {
                            api_key: Some(value),
                            ..crate::auth::types::ModelAuth::default()
                        },
                        env: None,
                        source: Some((*name).to_owned()),
                    }));
                }
            }
            Ok(None)
        })
    }
}

async fn generate_openrouter_images(
    model: ImagesModel,
    context: ImagesContext,
    options: Option<ImagesOptions>,
) -> AssistantImages {
    let api_model = api_model(&model);
    let api_options = options.as_ref().map(api_options);
    let result = api::generate_images(&api_model, &context, api_options.as_ref()).await;
    AssistantImages {
        api: result.api,
        provider: result.provider,
        model: result.model,
        output: result
            .output
            .into_iter()
            .map(|item| match item {
                api::ImagesContent::Text { text } => text,
                api::ImagesContent::Image { mime_type, data } => {
                    format!("data:{mime_type};base64,{data}")
                }
            })
            .collect(),
        stop_reason: match result.stop_reason {
            api::ImagesStopReason::Stop => "stop".to_owned(),
            api::ImagesStopReason::Error => "error".to_owned(),
            api::ImagesStopReason::Aborted => "aborted".to_owned(),
        },
        error_message: result.error_message,
    }
}

fn api_model(model: &ImagesModel) -> api::ImagesModel {
    let catalog = get_image_model(KnownImagesProvider::Openrouter, &model.id);
    api::ImagesModel {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        base_url: model
            .base_url
            .clone()
            .or_else(|| catalog.map(|model| model.base_url.to_owned()))
            .unwrap_or_default(),
        headers: api::ProviderHeaders::default(),
        output: catalog
            .map(|model| model.output)
            .unwrap_or(&[ImageModelContent::Image])
            .iter()
            .map(|content| match content {
                ImageModelContent::Text => api::ImagesOutputModality::Text,
                ImageModelContent::Image => api::ImagesOutputModality::Image,
            })
            .collect(),
        cost: catalog.map_or_else(api::UsageCostRates::default, |model| api::UsageCostRates {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        }),
    }
}

fn api_options(options: &ImagesOptions) -> api::ImagesOptions {
    api::ImagesOptions {
        api_key: options.api_key.clone(),
        headers: options.headers.clone().into_iter().collect(),
        on_payload: options.on_payload.clone(),
        on_response: options.on_response.clone(),
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
        signal: options.signal.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openrouter_images_provider() {
        let provider = openrouter_images_provider().expect("provider");
        assert_eq!(provider.id, OPENROUTER_IMAGES_PROVIDER_ID);
        assert_eq!(provider.name, OPENROUTER_IMAGES_PROVIDER_NAME);
        assert!(!provider.get_models().is_empty());
        assert!(provider.auth.api_key.is_some());
    }
}
