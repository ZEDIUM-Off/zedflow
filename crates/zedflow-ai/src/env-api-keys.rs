//! Environment API key lookup ported from Pi's `packages/ai/src/env-api-keys.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CACHED_VERTEX_ADC_CREDENTIALS_EXISTS: Mutex<Option<bool>> = Mutex::new(None);

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// Finds configured environment variable names that can provide an API key for a provider.
///
/// This reports actual API key variables only. Ambient credential sources such as AWS
/// profiles, AWS IAM credentials, and Google Application Default Credentials are excluded.
#[must_use]
pub fn find_env_keys(provider: &str, env: Option<&ProviderEnv>) -> Option<Vec<&'static str>> {
    let found: Vec<_> = api_key_env_vars(provider)
        .iter()
        .copied()
        .filter(|name| provider_env_value(name, env).is_some())
        .collect();

    (!found.is_empty()).then_some(found)
}

/// Gets the API key or authenticated marker for a provider from known environment variables.
#[must_use]
pub fn get_env_api_key(provider: &str, env: Option<&ProviderEnv>) -> Option<String> {
    if let Some(env_key) = find_env_keys(provider, env).and_then(|keys| keys.first().copied()) {
        return provider_env_value(env_key, env);
    }

    if provider == "google-vertex" {
        let has_credentials = has_vertex_adc_credentials(env);
        let has_project = provider_env_value("GOOGLE_CLOUD_PROJECT", env)
            .or_else(|| provider_env_value("GCLOUD_PROJECT", env))
            .is_some();
        let has_location = provider_env_value("GOOGLE_CLOUD_LOCATION", env).is_some();

        if has_credentials && has_project && has_location {
            return Some("<authenticated>".to_owned());
        }
    }

    if provider == "amazon-bedrock"
        && (provider_env_value("AWS_PROFILE", env).is_some()
            || (provider_env_value("AWS_ACCESS_KEY_ID", env).is_some()
                && provider_env_value("AWS_SECRET_ACCESS_KEY", env).is_some())
            || provider_env_value("AWS_BEARER_TOKEN_BEDROCK", env).is_some()
            || provider_env_value("AWS_CONTAINER_CREDENTIALS_RELATIVE_URI", env).is_some()
            || provider_env_value("AWS_CONTAINER_CREDENTIALS_FULL_URI", env).is_some()
            || provider_env_value("AWS_WEB_IDENTITY_TOKEN_FILE", env).is_some())
    {
        return Some("<authenticated>".to_owned());
    }

    None
}

fn provider_env_value(name: &str, env: Option<&ProviderEnv>) -> Option<String> {
    env.and_then(|env| env.get(name))
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

fn has_vertex_adc_credentials(env: Option<&ProviderEnv>) -> bool {
    if let Some(path) = env
        .and_then(|env| env.get("GOOGLE_APPLICATION_CREDENTIALS"))
        .filter(|value| !value.is_empty())
    {
        return Path::new(path).exists();
    }

    let mut cached = match CACHED_VERTEX_ADC_CREDENTIALS_EXISTS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some(exists) = *cached {
        return exists;
    }

    let exists = provider_env_value("GOOGLE_APPLICATION_CREDENTIALS", env)
        .map(|path| Path::new(&path).exists())
        .unwrap_or_else(|| default_adc_credentials_path().is_some_and(|path| path.exists()));
    *cached = Some(exists);
    exists
}

fn default_adc_credentials_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .map(|home| {
            home.join(".config")
                .join("gcloud")
                .join("application_default_credentials.json")
        })
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
        "google-vertex" => &["GOOGLE_CLOUD_API_KEY"],
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
        "cloudflare-workers-ai" | "cloudflare-ai-gateway" => &["CLOUDFLARE_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_anthropic_keys_in_precedence_order() {
        let env = ProviderEnv::from([
            ("ANTHROPIC_OAUTH_TOKEN".to_owned(), "oauth".to_owned()),
            ("ANTHROPIC_API_KEY".to_owned(), "api".to_owned()),
        ]);

        assert_eq!(
            find_env_keys("anthropic", Some(&env)),
            Some(vec!["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"])
        );
        assert_eq!(
            get_env_api_key("anthropic", Some(&env)),
            Some("oauth".to_owned())
        );
    }

    #[test]
    fn returns_bedrock_authenticated_for_ambient_credentials() {
        let env = ProviderEnv::from([
            ("AWS_ACCESS_KEY_ID".to_owned(), "key".to_owned()),
            ("AWS_SECRET_ACCESS_KEY".to_owned(), "secret".to_owned()),
        ]);

        assert_eq!(
            get_env_api_key("amazon-bedrock", Some(&env)),
            Some("<authenticated>".to_owned())
        );
        assert_eq!(find_env_keys("amazon-bedrock", Some(&env)), None);
    }

    #[test]
    fn resolves_fireworks_api_key_from_environment() {
        let env = ProviderEnv::from([(
            "FIREWORKS_API_KEY".to_owned(),
            "test-fireworks-key".to_owned(),
        )]);

        assert_eq!(
            find_env_keys("fireworks", Some(&env)),
            Some(vec!["FIREWORKS_API_KEY"])
        );
        assert_eq!(
            get_env_api_key("fireworks", Some(&env)),
            Some("test-fireworks-key".to_owned())
        );
    }

    #[test]
    fn returns_none_for_unknown_provider() {
        assert_eq!(get_env_api_key("unknown", None), None);
        assert_eq!(find_env_keys("unknown", None), None);
    }
}
