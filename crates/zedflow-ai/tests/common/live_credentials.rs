//! Live provider capability detection for credential-gated tests.
//!
//! Live tests should call these helpers and skip only when the named provider
//! capability is unavailable. Messages name missing sources, never values.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCapability {
    pub provider: String,
    pub available: bool,
    pub source: Option<CredentialSource>,
    pub missing_sources: Vec<String>,
}

impl LiveCapability {
    #[must_use]
    pub fn skip_message(&self) -> Option<String> {
        if self.available {
            None
        } else {
            Some(format!(
                "skipping live {} tests: missing provider capability ({})",
                self.provider,
                self.missing_sources.join(" or ")
            ))
        }
    }

    pub fn require(&self) -> Result<(), String> {
        self.skip_message().map_or(Ok(()), Err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    EnvVar(&'static str),
    PiAuthJsonApiKey,
    PiAuthJsonOAuth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCredentialConfig {
    pub auth_path: PathBuf,
}

impl Default for LiveCredentialConfig {
    fn default() -> Self {
        Self {
            auth_path: default_pi_auth_path(),
        }
    }
}

#[must_use]
pub fn openrouter() -> LiveCapability {
    capability("openrouter")
}

#[must_use]
pub fn openai_codex() -> LiveCapability {
    capability("openai-codex")
}

#[must_use]
pub fn api_key(provider: &str) -> Option<String> {
    api_key_with_config(provider, &LiveCredentialConfig::default())
}

#[must_use]
pub fn api_key_with_config(provider: &str, config: &LiveCredentialConfig) -> Option<String> {
    env_credential(provider)
        .map(|credential| credential.value)
        .or_else(|| {
            auth_json_credential(provider, &config.auth_path).map(|credential| credential.value)
        })
}

#[must_use]
pub fn capability(provider: &str) -> LiveCapability {
    capability_with_config(provider, &LiveCredentialConfig::default())
}

#[must_use]
pub fn capability_with_config(provider: &str, config: &LiveCredentialConfig) -> LiveCapability {
    if let Some(credential) =
        env_credential(provider).or_else(|| auth_json_credential(provider, &config.auth_path))
    {
        return LiveCapability {
            provider: provider.to_owned(),
            available: true,
            source: Some(credential.source),
            missing_sources: Vec::new(),
        };
    }

    let env_vars = api_key_env_vars(provider);

    let mut missing_sources: Vec<String> =
        env_vars.iter().map(|name| format!("env {name}")).collect();
    missing_sources.push(format!(
        "{} entry for {provider}",
        config.auth_path.display()
    ));

    LiveCapability {
        provider: provider.to_owned(),
        available: false,
        source: None,
        missing_sources,
    }
}

#[must_use]
pub fn api_key_env_vars(provider: &str) -> &'static [&'static str] {
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
        // Pi's Codex live tests resolve this from ~/.pi/agent/auth.json OAuth storage.
        "openai-codex" => &[],
        _ => &[],
    }
}

#[must_use]
pub fn default_pi_auth_path() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
        .join("auth.json")
}

fn env_credential(provider: &str) -> Option<ResolvedCredential> {
    for env_var in api_key_env_vars(provider) {
        let Ok(value) = env::var(env_var) else {
            continue;
        };
        if !value.trim().is_empty() {
            return Some(ResolvedCredential {
                source: CredentialSource::EnvVar(env_var),
                value,
            });
        }
    }
    None
}

fn auth_json_credential(provider: &str, path: &PathBuf) -> Option<ResolvedCredential> {
    let content = fs::read_to_string(path).ok()?;
    let storage: BTreeMap<String, StoredCredential> = serde_json::from_str(&content).ok()?;
    match storage.get(provider)? {
        StoredCredential::ApiKey { key }
            if key.as_deref().is_some_and(|key| !key.trim().is_empty()) =>
        {
            Some(ResolvedCredential {
                source: CredentialSource::PiAuthJsonApiKey,
                value: key.clone().expect("checked as Some above"),
            })
        }
        StoredCredential::OAuth { access } if !access.trim().is_empty() => {
            Some(ResolvedCredential {
                source: CredentialSource::PiAuthJsonOAuth,
                value: access.clone(),
            })
        }
        _ => None,
    }
}

struct ResolvedCredential {
    source: CredentialSource,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StoredCredential {
    #[serde(rename = "api_key")]
    ApiKey { key: Option<String> },
    #[serde(rename = "oauth")]
    OAuth { access: String },
}
