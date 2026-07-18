//! Shared static provider catalog helpers.

use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{
    Context, CreateProviderOptions, Model, Provider, ProviderApi, ProviderAuth, create_provider,
    terminal_stream_error,
};
use crate::types::{ModelCost, ModelInput, ProviderStreams, SimpleStreamOptions, StreamOptions};

pub(crate) fn static_provider(id: &str, name: &str, models: Vec<Model>) -> Provider {
    let base_url = models
        .iter()
        .map(|model| model.base_url.as_str())
        .find(|base_url| !base_url.is_empty())
        .map(str::to_owned);
    let api = provider_api_for_models(&models);
    create_provider(CreateProviderOptions {
        id: id.into(),
        name: Some(name.into()),
        base_url,
        headers: None,
        auth: provider_auth_for_id(id),
        models,
        refresh_models: None,
        api,
    })
}

fn provider_auth_for_id(id: &str) -> ProviderAuth {
    let api_key = api_key_auth_for_id(id);
    let oauth = match id {
        "anthropic" => Some(Arc::new(crate::utils::oauth::anthropic::ANTHROPIC_OAUTH)
            as Arc<dyn crate::auth::types::OAuthAuth>),
        "github-copilot" => Some(
            Arc::new(crate::utils::oauth::github_copilot::GITHUB_COPILOT_OAUTH)
                as Arc<dyn crate::auth::types::OAuthAuth>,
        ),
        "openai-codex" => Some(
            Arc::new(crate::utils::oauth::openai_codex::OPENAI_CODEX_OAUTH)
                as Arc<dyn crate::auth::types::OAuthAuth>,
        ),
        _ => None,
    };

    ProviderAuth { api_key, oauth }
}

fn api_key_auth_for_id(id: &str) -> Option<Arc<dyn crate::auth::types::ApiKeyAuth>> {
    match id {
        "amazon-bedrock" => Some(Arc::new(AmbientEnvAuth {
            name: "AWS credentials".to_owned(),
            env_vars: crate::providers::amazon_bedrock::BEDROCK_AUTH_ENV,
        })),
        "cloudflare-workers-ai" => Some(Arc::new(
            crate::providers::cloudflare_auth::cloudflare_workers_ai_auth(),
        )),
        "cloudflare-ai-gateway" => Some(Arc::new(
            crate::providers::cloudflare_auth::cloudflare_ai_gateway_auth(),
        )),
        "google-vertex" => Some(Arc::new(GoogleVertexAuth)),
        _ => {
            let env_vars = api_key_env_vars(id);
            (!env_vars.is_empty()).then(|| {
                Arc::new(EnvApiKeyAuth {
                    name: format!("{id} API key"),
                    env_vars,
                }) as Arc<dyn crate::auth::types::ApiKeyAuth>
            })
        }
    }
}

fn api_key_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "github-copilot" => &["COPILOT_GITHUB_TOKEN"],
        "anthropic" => &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"],
        "ant-ling" => &["ANT_LING_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "azure-openai-responses" => &["AZURE_OPENAI_API_KEY"],
        "nvidia" => &["NVIDIA_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY"],
        "google" => &["GEMINI_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "cerebras" => &["CEREBRAS_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => &["AI_GATEWAY_API_KEY"],
        "zai" => &["ZAI_API_KEY"],
        "zai-coding-cn" => &["ZAI_CODING_CN_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "minimax" => &["MINIMAX_API_KEY"],
        "minimax-cn" => &["MINIMAX_CN_API_KEY"],
        "moonshotai" | "moonshotai-cn" => &["MOONSHOT_API_KEY"],
        "huggingface" => &["HF_TOKEN"],
        "fireworks" => &["FIREWORKS_API_KEY"],
        "together" => &["TOGETHER_API_KEY"],
        "opencode" | "opencode-go" => &["OPENCODE_API_KEY"],
        "kimi-coding" => &["KIMI_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        _ => &[],
    }
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

#[derive(Debug)]
struct AmbientEnvAuth {
    name: String,
    env_vars: &'static [&'static str],
}

impl crate::auth::types::ApiKeyAuth for AmbientEnvAuth {
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
            for name in self.env_vars {
                if input.ctx.env(name).await.is_some() {
                    return Ok(Some(crate::auth::types::ResolvedAuth {
                        auth: crate::auth::types::ModelAuth::default(),
                        env: None,
                        source: Some((*name).to_owned()),
                    }));
                }
            }
            Ok(None)
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct GoogleVertexAuth;

impl crate::auth::types::ApiKeyAuth for GoogleVertexAuth {
    fn name(&self) -> &str {
        "Google Cloud credentials"
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
                .ctx
                .env("GOOGLE_CLOUD_API_KEY")
                .await
                .filter(|key| !key.is_empty())
            {
                return Ok(Some(crate::auth::types::ResolvedAuth {
                    auth: crate::auth::types::ModelAuth {
                        api_key: Some(key),
                        ..crate::auth::types::ModelAuth::default()
                    },
                    env: None,
                    source: Some("GOOGLE_CLOUD_API_KEY".to_owned()),
                }));
            }

            let explicit_adc = input.ctx.env("GOOGLE_APPLICATION_CREDENTIALS").await;
            let has_adc = match explicit_adc.as_deref() {
                Some(path) if !path.is_empty() => input.ctx.file_exists(path).await,
                _ => {
                    input
                        .ctx
                        .file_exists("~/.config/gcloud/application_default_credentials.json")
                        .await
                }
            };
            let project = match input.ctx.env("GOOGLE_CLOUD_PROJECT").await {
                Some(project) => Some(project),
                None => input.ctx.env("GCLOUD_PROJECT").await,
            };
            let location = input.ctx.env("GOOGLE_CLOUD_LOCATION").await;
            if has_adc && project.is_some() && location.is_some() {
                return Ok(Some(crate::auth::types::ResolvedAuth {
                    auth: crate::auth::types::ModelAuth::default(),
                    env: None,
                    source: Some("application default credentials".to_owned()),
                }));
            }
            Ok(None)
        })
    }
}

fn provider_api_for_models(models: &[Model]) -> ProviderApi {
    let mut apis = models
        .iter()
        .map(|model| model.api.as_str())
        .filter(|api| !api.is_empty())
        .collect::<Vec<_>>();
    apis.sort_unstable();
    apis.dedup();

    if apis.len() <= 1 {
        let api = apis.first().copied().unwrap_or("unknown");
        return ProviderApi::Single(
            ready_builtin_provider_streams(api).unwrap_or_else(|| unavailable_streams(api)),
        );
    }

    ProviderApi::ByApi(
        apis.into_iter()
            .map(|api| {
                (
                    api.to_owned(),
                    ready_builtin_provider_streams(api).unwrap_or_else(|| unavailable_streams(api)),
                )
            })
            .collect::<HashMap<_, _>>(),
    )
}

pub(crate) fn ready_builtin_provider_streams(api: &str) -> Option<ProviderStreams> {
    match api {
        "anthropic-messages" => Some(crate::api::anthropic_messages_lazy::anthropic_messages_api()),
        "openai-completions" => {
            Some(crate::api::openai_completions_lazy::open_ai_completions_api())
        }
        "openai-responses" => Some(crate::api::openai_responses_lazy::open_ai_responses_api()),
        "openai-codex-responses" => {
            Some(crate::api::openai_codex_responses_lazy::open_ai_codex_responses_api())
        }
        "azure-openai-responses" => {
            Some(crate::api::azure_openai_responses_lazy::azure_open_ai_responses_api())
        }
        "google-generative-ai" => {
            Some(crate::api::google_generative_ai_lazy::google_generative_ai_api())
        }
        "google-vertex" => Some(crate::api::google_vertex_lazy::google_vertex_api()),
        "mistral-conversations" => {
            Some(crate::api::mistral_conversations_lazy::mistral_conversations_api())
        }
        "bedrock-converse-stream" => {
            Some(crate::api::bedrock_converse_stream_lazy::bedrock_converse_stream_api())
        }
        _ => None,
    }
}

fn unavailable_streams(api: &str) -> ProviderStreams {
    let api = api.to_owned();
    let simple_api = api.clone();
    ProviderStreams {
        stream: Arc::new(
            move |model: &Model, _context: &Context, _options: Option<&StreamOptions>| {
                terminal_stream_error(
                    model,
                    format!("No Rust transport is available for API \"{api}\""),
                )
            },
        ),
        stream_simple: Arc::new(
            move |model: &Model, _context: &Context, _options: Option<&SimpleStreamOptions>| {
                terminal_stream_error(
                    model,
                    format!("No Rust transport is available for API \"{simple_api}\""),
                )
            },
        ),
    }
}

pub(crate) fn models_from_catalog<T>(catalog: &[T]) -> Vec<Model>
where
    T: CatalogModel,
{
    catalog
        .iter()
        .map(|model| Model {
            provider: model.provider().into(),
            id: model.id().into(),
            api: model.api().into(),
            name: model.name().into(),
            base_url: model.base_url().into(),
            reasoning: model.reasoning(),
            input: model.input(),
            cost: model.cost(),
            context_window: model.context_window(),
            max_tokens: model.max_tokens(),
            ..Model::default()
        })
        .collect()
}

pub(crate) trait CatalogModel {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn api(&self) -> &'static str;
    fn provider(&self) -> &'static str;
    fn base_url(&self) -> &'static str;
    fn reasoning(&self) -> bool;
    fn input(&self) -> Vec<ModelInput>;
    fn cost(&self) -> ModelCost;
    fn context_window(&self) -> u64;
    fn max_tokens(&self) -> u64;
}

fn input_kind(input: &&str) -> ModelInput {
    match *input {
        "image" => ModelInput::Image,
        _ => ModelInput::Text,
    }
}

macro_rules! impl_catalog_model {
    ($($ty:path),+ $(,)?) => {
        $(
            impl CatalogModel for $ty {
                fn id(&self) -> &'static str { self.id }
                fn name(&self) -> &'static str { self.name }
                fn api(&self) -> &'static str { self.api }
                fn provider(&self) -> &'static str { self.provider }
                fn base_url(&self) -> &'static str { self.base_url }
                fn reasoning(&self) -> bool { self.reasoning }
                fn input(&self) -> Vec<ModelInput> { self.input.iter().map(input_kind).collect() }
                fn cost(&self) -> ModelCost {
                    ModelCost {
                        input: self.cost.input,
                        output: self.cost.output,
                        cache_read: self.cost.cache_read,
                        cache_write: self.cost.cache_write,
                    }
                }
                fn context_window(&self) -> u64 { self.context_window.into() }
                fn max_tokens(&self) -> u64 { self.max_tokens.into() }
            }
        )+
    };
}

impl_catalog_model!(
    crate::providers::cerebras_models::CerebrasModel,
    crate::providers::cloudflare_workers_ai_models::CloudflareWorkersAiModel,
    crate::providers::fireworks_models::FireworksModel,
    crate::providers::github_copilot_models::GithubCopilotModel,
    crate::providers::google_models::GoogleModel,
    crate::providers::groq_models::GroqModel,
    crate::providers::huggingface_models::HuggingFaceModel,
    crate::providers::minimax_models::MiniMaxModel,
    crate::providers::mistral_models::MistralModel,
    crate::providers::nvidia_models::NvidiaModel,
    crate::providers::openai_codex_models::OpenAICodexModel,
    crate::providers::openai_models::OpenAiModel,
    crate::providers::opencode_go_models::OpenCodeGoModel,
    crate::providers::opencode_models::OpenCodeModel,
    crate::providers::openrouter_models::OpenRouterModel,
    crate::providers::together_models::TogetherModel,
    crate::providers::vercel_ai_gateway_models::VercelAiGatewayModel,
    crate::providers::xai_models::XaiModel,
    crate::providers::xiaomi_models::XiaomiModel,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ready_builtin_model_resolves_in_the_dispatch_table() {
        for provider in crate::providers::all::builtin_providers() {
            for model in provider.get_models() {
                assert!(
                    ready_builtin_provider_streams(&model.api).is_some(),
                    "{} model {} has no transport for {}",
                    provider.id,
                    model.id,
                    model.api
                );
                if let ProviderApi::ByApi(apis) = &provider.api {
                    assert!(
                        apis.contains_key(&model.api),
                        "{} does not dispatch {}",
                        provider.id,
                        model.api
                    );
                }
            }
        }

        assert!(ready_builtin_provider_streams("openai-codex-responses").is_some());
        assert!(ready_builtin_provider_streams("bedrock-converse-stream").is_some());
        assert!(ready_builtin_provider_streams("custom-api").is_none());
    }
}
