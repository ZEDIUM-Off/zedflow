//! OpenRouter image provider factory ported from Pi's `packages/ai/src/providers/openrouter-images.ts`.

use std::sync::Arc;

use crate::error::Result;

use crate::api::openrouter_images as api;
use crate::image_models::{KnownImagesProvider, get_image_models};
use crate::image_models_generated::ImageModelContent;
use crate::images_models::{
    AssistantImages, CreateImagesProviderOptions, ImagesContext, ImagesModel, ImagesOptions,
    ImagesProvider, ImagesStopReason, ProviderAuth, create_images_provider,
};
use crate::types::{
    ImageContent, ImageContentType, ModelInput, ModelOutput, TextContent, TextContentType,
    ToolResultContentBlock, Usage, UsageCost,
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
            .map(canonical_model)
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
    let api_context = api_context(&context);
    let api_options = options.as_ref().map(|options| api_options(options, &model));
    let result = api::generate_images(&api_model, &api_context, api_options.as_ref()).await;
    canonical_result(result)
}

fn canonical_result(result: api::AssistantImages) -> AssistantImages {
    AssistantImages {
        api: result.api,
        provider: result.provider,
        model: result.model,
        output: result.output.into_iter().map(canonical_content).collect(),
        response_id: result.response_id,
        usage: result.usage.map(canonical_usage),
        stop_reason: match result.stop_reason {
            api::ImagesStopReason::Stop => ImagesStopReason::Stop,
            api::ImagesStopReason::Error => ImagesStopReason::Error,
            api::ImagesStopReason::Aborted => ImagesStopReason::Aborted,
        },
        error_message: result.error_message,
        timestamp: result.timestamp,
    }
}

fn canonical_model(model: &crate::image_models_generated::ImageModel) -> ImagesModel {
    ImagesModel {
        id: model.id.to_owned(),
        name: model.name.to_owned(),
        api: model.api.to_owned(),
        provider: model.provider.to_owned(),
        base_url: model.base_url.to_owned(),
        input: model.input.iter().map(canonical_input).collect(),
        output: model.output.iter().map(canonical_output).collect(),
        cost: crate::types::ModelCost {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
        headers: None,
    }
}

fn canonical_input(content: &ImageModelContent) -> ModelInput {
    match content {
        ImageModelContent::Text => ModelInput::Text,
        ImageModelContent::Image => ModelInput::Image,
    }
}

fn canonical_output(content: &ImageModelContent) -> ModelOutput {
    match content {
        ImageModelContent::Text => ModelOutput::Text,
        ImageModelContent::Image => ModelOutput::Image,
    }
}

fn api_model(model: &ImagesModel) -> api::ImagesModel {
    api::ImagesModel {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        base_url: model.base_url.clone(),
        headers: model
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| (name, Some(value)))
            .collect(),
        output: model
            .output
            .iter()
            .map(|content| match content {
                ModelOutput::Text => api::ImagesOutputModality::Text,
                ModelOutput::Image => api::ImagesOutputModality::Image,
            })
            .collect(),
        cost: api::UsageCostRates {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
    }
}

fn api_context(context: &ImagesContext) -> api::ImagesContext {
    api::ImagesContext {
        input: context
            .input
            .iter()
            .map(|content| match content {
                crate::types::UserContentBlock::Text(text) => api::ImagesContent::Text {
                    text: text.text.clone(),
                },
                crate::types::UserContentBlock::Image(image) => api::ImagesContent::Image {
                    mime_type: image.mime_type.clone(),
                    data: image.data.clone(),
                },
            })
            .collect(),
    }
}

fn canonical_content(content: api::ImagesContent) -> ToolResultContentBlock {
    match content {
        api::ImagesContent::Text { text } => ToolResultContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text,
            text_signature: None,
        }),
        api::ImagesContent::Image { mime_type, data } => {
            ToolResultContentBlock::Image(ImageContent {
                content_type: ImageContentType::Image,
                data,
                mime_type,
            })
        }
    }
}

fn canonical_usage(usage: api::Usage) -> Usage {
    Usage {
        input: usage.input,
        output: usage.output,
        cache_read: usage.cache_read,
        cache_write: usage.cache_write,
        cache_write_1h: None,
        reasoning: None,
        total_tokens: usage.total_tokens,
        cost: UsageCost {
            input: usage.cost.input,
            output: usage.cost.output,
            cache_read: usage.cost.cache_read,
            cache_write: usage.cost.cache_write,
            total: usage.cost.total,
        },
    }
}

fn api_options(options: &ImagesOptions, model: &ImagesModel) -> api::ImagesOptions {
    let payload_model = model.clone();
    let on_payload = options.on_payload.clone().map(|hook| {
        Arc::new(move |payload, _model: &api::ImagesModel| hook(payload, payload_model.clone()))
            as api::ImagesPayloadHook
    });
    let response_model = model.clone();
    let on_response = options.on_response.clone().map(|hook| {
        Arc::new(
            move |response: api::OpenRouterImagesResponseMeta, _model: &api::ImagesModel| {
                hook(
                    crate::types::ProviderResponse {
                        status: response.status,
                        headers: response.headers.into_iter().collect(),
                    },
                    response_model.clone(),
                )
            },
        ) as api::ImagesResponseHook
    });
    api::ImagesOptions {
        api_key: options.api_key.clone(),
        headers: options
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        on_payload,
        on_response,
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

    #[test]
    fn adapter_preserves_custom_model_and_complete_result() {
        let model = ImagesModel {
            id: "custom/image-model".into(),
            name: "Custom".into(),
            api: OPENROUTER_IMAGES_API.into(),
            provider: OPENROUTER_IMAGES_PROVIDER_ID.into(),
            base_url: "https://custom.example/v1".into(),
            input: vec![ModelInput::Text, ModelInput::Image],
            output: vec![ModelOutput::Text, ModelOutput::Image],
            cost: crate::types::ModelCost {
                input: 1.0,
                output: 2.0,
                cache_read: 3.0,
                cache_write: 4.0,
            },
            headers: Some(std::collections::HashMap::from([(
                "x-custom".into(),
                "yes".into(),
            )])),
        };
        let adapted = api_model(&model);
        assert_eq!(adapted.id, model.id);
        assert_eq!(adapted.base_url, model.base_url);
        assert_eq!(adapted.output.len(), 2);
        assert_eq!(adapted.cost.cache_write, 4.0);

        let result = canonical_result(api::AssistantImages {
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            output: vec![
                api::ImagesContent::Text {
                    text: "done".into(),
                },
                api::ImagesContent::Image {
                    mime_type: "image/png".into(),
                    data: "aW1hZ2U=".into(),
                },
            ],
            response_id: Some("response-1".into()),
            usage: Some(api::Usage {
                input: 5,
                output: 6,
                cache_read: 7,
                cache_write: 8,
                total_tokens: 26,
                cost: api::UsageCost {
                    input: 0.1,
                    output: 0.2,
                    cache_read: 0.3,
                    cache_write: 0.4,
                    total: 1.0,
                },
            }),
            stop_reason: api::ImagesStopReason::Stop,
            error_message: None,
            timestamp: 123,
        });

        assert_eq!(result.response_id.as_deref(), Some("response-1"));
        assert_eq!(result.output.len(), 2);
        assert_eq!(
            result.usage.as_ref().map(|usage| usage.total_tokens),
            Some(26)
        );
        assert_eq!(
            result.usage.as_ref().map(|usage| usage.cost.total),
            Some(1.0)
        );
        assert_eq!(result.stop_reason, ImagesStopReason::Stop);
        assert_eq!(result.timestamp, 123);
    }
}
