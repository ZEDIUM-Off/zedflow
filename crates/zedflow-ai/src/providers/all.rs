//! Built-in provider helpers ported from Pi's `packages/ai/src/providers/all.ts`.

use crate::images_models::{ImagesProvider, create_images_models};
use crate::models::{Model, Models, Provider, create_models};
use crate::models_generated::BUILTIN_PROVIDERS;
use crate::providers::{
    amazon_bedrock::amazon_bedrock_provider, ant_ling::ant_ling_provider,
    anthropic::anthropic_provider, azure_openai_responses::azure_openai_responses_provider,
    cerebras::cerebras_provider, cloudflare_ai_gateway::cloudflare_ai_gateway_provider,
    cloudflare_workers_ai::cloudflare_workers_ai_provider, deepseek::deepseek_provider,
    fireworks::fireworks_provider, github_copilot::github_copilot_provider,
    google::google_provider, google_vertex::google_vertex_provider, groq::groq_provider,
    huggingface::huggingface_provider, kimi_coding::kimi_coding_provider,
    minimax::minimax_provider, minimax_cn::minimax_cn_provider, mistral::mistral_provider,
    moonshotai::moonshotai_provider, moonshotai_cn::moonshotai_cn_provider,
    nvidia::nvidia_provider, openai::openai_provider, openai_codex::openai_codex_provider,
    opencode::opencode_provider, opencode_go::opencode_go_provider,
    openrouter::openrouter_provider, openrouter_images::openrouter_images_provider,
    together::together_provider, vercel_ai_gateway::vercel_ai_gateway_provider, xai::xai_provider,
    xiaomi::xiaomi_provider, xiaomi_token_plan_ams::xiaomi_token_plan_ams_provider,
    xiaomi_token_plan_cn::xiaomi_token_plan_cn_provider,
    xiaomi_token_plan_sgp::xiaomi_token_plan_sgp_provider, zai::zai_provider,
    zai_coding_cn::zai_coding_cn_provider,
};

/// Typed read of the generated built-in catalog.
#[must_use]
pub fn get_builtin_model(provider: &str, model_id: &str) -> Option<Model> {
    get_builtin_models(provider)
        .into_iter()
        .find(|model| model.id == model_id)
}

/// Built-in provider ids from the generated catalog.
#[must_use]
pub fn get_builtin_providers() -> Vec<&'static str> {
    BUILTIN_PROVIDERS.to_vec()
}

/// Built-in models for a provider.
#[must_use]
pub fn get_builtin_models(provider: &str) -> Vec<Model> {
    builtin_provider(provider).map_or_else(Vec::new, |provider| provider.get_models())
}

/// All built-in providers, freshly constructed.
#[must_use]
pub fn builtin_providers() -> Vec<Provider> {
    vec![
        amazon_bedrock_provider(),
        ant_ling_provider(),
        anthropic_provider().expect("static anthropic provider"),
        azure_openai_responses_provider(),
        cerebras_provider().expect("static cerebras provider"),
        cloudflare_ai_gateway_provider().expect("static cloudflare ai gateway provider"),
        cloudflare_workers_ai_provider(),
        deepseek_provider().expect("static deepseek provider"),
        fireworks_provider().expect("static fireworks provider"),
        github_copilot_provider().expect("static github copilot provider"),
        google_provider().expect("static google provider"),
        google_vertex_provider().expect("static google vertex provider"),
        groq_provider().expect("static groq provider"),
        huggingface_provider().expect("static huggingface provider"),
        kimi_coding_provider().expect("static kimi coding provider"),
        minimax_provider().expect("static minimax provider"),
        minimax_cn_provider().expect("static minimax cn provider"),
        mistral_provider().expect("static mistral provider"),
        moonshotai_provider().expect("static moonshotai provider"),
        moonshotai_cn_provider().expect("static moonshotai cn provider"),
        nvidia_provider().expect("static nvidia provider"),
        openai_provider().expect("static openai provider"),
        openai_codex_provider().expect("static openai codex provider"),
        opencode_provider().expect("static opencode provider"),
        opencode_go_provider().expect("static opencode go provider"),
        openrouter_provider().expect("static openrouter provider"),
        together_provider().expect("static together provider"),
        vercel_ai_gateway_provider().expect("static vercel ai gateway provider"),
        xai_provider().expect("static xai provider"),
        xiaomi_provider().expect("static xiaomi provider"),
        xiaomi_token_plan_ams_provider().expect("static xiaomi token plan ams provider"),
        xiaomi_token_plan_cn_provider().expect("static xiaomi token plan cn provider"),
        xiaomi_token_plan_sgp_provider().expect("static xiaomi token plan sgp provider"),
        zai_provider().expect("static zai provider"),
        zai_coding_cn_provider().expect("static zai coding cn provider"),
    ]
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
    vec![openrouter_images_provider().expect("static openrouter image provider")]
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

/// Built-in image-generation providers with a custom auth context.
#[must_use]
pub fn builtin_images_models_with_auth_context(
    context: impl crate::auth::types::AuthContext + 'static,
) -> crate::images_models::ImagesModels {
    let mut models = crate::images_models::create_images_models_with_auth_context(context);
    for provider in builtin_images_providers() {
        models.set_provider(provider);
    }
    models
}

fn builtin_provider(provider: &str) -> Option<Provider> {
    builtin_providers()
        .into_iter()
        .find(|entry| entry.id == provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_builtin_provider_keys() {
        assert!(get_builtin_providers().contains(&"openai"));
    }

    #[test]
    fn returns_builtin_models_in_provider_order() {
        let openai = get_builtin_models("openai");
        assert_eq!(
            openai.first().map(|model| model.provider.as_str()),
            Some("openai")
        );
        assert!(get_builtin_model("openai", "gpt-4").is_some());
    }
}
