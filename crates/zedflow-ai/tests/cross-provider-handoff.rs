//! Port of Pi `packages/ai/test/cross-provider-handoff.test.ts`.
//!
//! This is an integration/live-provider parity test. It is ignored because the source test
//! intentionally calls real providers, and the Rust compat provider catalog/dispatch path is
//! still represented by port placeholders.

use std::collections::HashMap;
use std::fs::write;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use zedflow_ai::compat::get_model;
use zedflow_ai::env_api_keys::get_env_api_key;
use zedflow_ai::types::{Api, Message, Tool};

const BLOCKER: &str = "live cross-provider calls skipped; compat::get_model/completeSimple, OAuth resolveApiKey, builtin provider dispatch, and provider stream implementations are still request-capture blockers";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderModelPair {
    provider: &'static str,
    model: &'static str,
    label: &'static str,
    api_override: Option<&'static str>,
    upstream_api_key_env: Option<&'static str>,
}

const PROVIDER_MODEL_PAIRS: &[ProviderModelPair] = &[
    ProviderModelPair {
        provider: "anthropic",
        model: "claude-sonnet-4-5",
        label: "anthropic-claude-sonnet-4-5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "google",
        model: "gemini-3-flash-preview",
        label: "google-gemini-3-flash-preview",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "openai",
        model: "gpt-4o-mini",
        label: "openai-completions-gpt-4o-mini",
        api_override: Some("openai-completions"),
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "openai",
        model: "gpt-5-mini",
        label: "openai-responses-gpt-5-mini",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        label: "azure-openai-responses-gpt-4o-mini",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "openai-codex",
        model: "gpt-5.5",
        label: "openai-codex-gpt-5.5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "github-copilot",
        model: "claude-sonnet-4.5",
        label: "copilot-claude-sonnet-4.5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "github-copilot",
        model: "gpt-5.1-codex",
        label: "copilot-gpt-5.1-codex",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "github-copilot",
        model: "gemini-3-flash-preview",
        label: "copilot-gemini-3-flash-preview",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "github-copilot",
        model: "grok-code-fast-1",
        label: "copilot-grok-code-fast-1",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        label: "bedrock-claude-sonnet-4-5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "xai",
        model: "grok-code-fast-1",
        label: "xai-grok-code-fast-1",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "cerebras",
        model: "zai-glm-4.7",
        label: "cerebras-zai-glm-4.7",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        label: "cloudflare-kimi-k2.6",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        label: "cloudflare-gateway-kimi-k2.6",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "cloudflare-ai-gateway",
        model: "claude-sonnet-4-5",
        label: "cloudflare-gateway-claude-sonnet-4-5",
        api_override: None,
        upstream_api_key_env: Some("ANTHROPIC_API_KEY"),
    },
    ProviderModelPair {
        provider: "cloudflare-ai-gateway",
        model: "gpt-5.1",
        label: "cloudflare-gateway-gpt-5.1",
        api_override: None,
        upstream_api_key_env: Some("OPENAI_API_KEY"),
    },
    ProviderModelPair {
        provider: "groq",
        model: "openai/gpt-oss-120b",
        label: "groq-gpt-oss-120b",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        label: "huggingface-kimi-k2.5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        label: "together-kimi-k2.6",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        label: "kimi-coding-k2-thinking",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "mistral",
        model: "devstral-medium-latest",
        label: "mistral-devstral-medium",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "minimax",
        model: "MiniMax-M2.7",
        label: "minimax-m2.7",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "minimax-cn",
        model: "MiniMax-M2.7",
        label: "minimax-m2.7",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "big-pickle",
        label: "zen-big-pickle",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "claude-sonnet-4-5",
        label: "zen-claude-sonnet-4-5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "gemini-3-flash",
        label: "zen-gemini-3-flash",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "glm-4.7-free",
        label: "zen-glm-4.7-free",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "gpt-5.2-codex",
        label: "zen-gpt-5.2-codex",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode",
        model: "minimax-m2.1-free",
        label: "zen-minimax-m2.1-free",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode-go",
        model: "kimi-k2.5",
        label: "go-kimi-k2.5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "opencode-go",
        model: "minimax-m2.5",
        label: "go-minimax-m2.5",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        label: "xiaomi-mimo-v2.5-pro",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        label: "xiaomi-token-plan-cn-mimo-v2.5-pro",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        label: "xiaomi-token-plan-ams-mimo-v2.5-pro",
        api_override: None,
        upstream_api_key_env: None,
    },
    ProviderModelPair {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        label: "xiaomi-token-plan-sgp-mimo-v2.5-pro",
        api_override: None,
        upstream_api_key_env: None,
    },
];

#[allow(
    dead_code,
    reason = "fields are consumed only by capability-gated handoff dispatch"
)]
#[derive(Debug, Clone)]
struct CachedContext {
    label: &'static str,
    provider: &'static str,
    model: &'static str,
    api: Api,
    messages: Vec<Message>,
    generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandoffResult {
    target: &'static str,
    success: bool,
    error: Option<String>,
}

fn test_tool() -> Tool {
    Tool {
        name: "double_number".to_owned(),
        description: "Doubles a number and returns the result".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "value": {
                    "type": "number",
                    "description": "A number to double"
                }
            },
            "required": ["value"]
        }),
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn has_env_value(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn get_api_key(provider: &str) -> Option<String> {
    // references/pi/packages/ai/test/oauth.ts resolveApiKey(provider).
    get_env_api_key(provider, None)
}

fn has_azure_openai_credentials() -> bool {
    has_env_value("AZURE_OPENAI_API_KEY")
        && (has_env_value("AZURE_OPENAI_BASE_URL") || has_env_value("AZURE_OPENAI_RESOURCE_NAME"))
}

fn has_cloudflare_workers_ai_credentials() -> bool {
    has_env_value("CLOUDFLARE_API_KEY") && has_env_value("CLOUDFLARE_ACCOUNT_ID")
}

fn has_cloudflare_ai_gateway_credentials() -> bool {
    has_cloudflare_workers_ai_credentials() && has_env_value("CLOUDFLARE_GATEWAY_ID")
}

fn has_api_key(pair: &ProviderModelPair) -> bool {
    match pair.provider {
        "azure-openai-responses" => has_azure_openai_credentials(),
        "cloudflare-workers-ai" => has_cloudflare_workers_ai_credentials(),
        "cloudflare-ai-gateway" => {
            has_cloudflare_ai_gateway_credentials()
                && pair.upstream_api_key_env.is_none_or(has_env_value)
        }
        provider => get_env_api_key(provider, None).is_some(),
    }
}

fn get_headers(pair: &ProviderModelPair) -> Option<HashMap<String, String>> {
    let upstream_api_key = std::env::var(pair.upstream_api_key_env?).ok()?;
    (!upstream_api_key.is_empty()).then(|| {
        HashMap::from([(
            "Authorization".to_owned(),
            format!("Bearer {upstream_api_key}"),
        )])
    })
}

fn has_any_api_key() -> bool {
    PROVIDER_MODEL_PAIRS.iter().any(has_api_key)
}

fn dump_failure_payload(label: &str, error: &str, payload: Option<&Value>, messages: &[Message]) {
    let filename = format!("/tmp/pi-handoff-{label}-{}.json", now_millis());
    let body = json!({
        "label": label,
        "error": error,
        "payload": payload,
        "messages": messages,
    });
    let _ = write(
        filename,
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    );
}

fn generate_context(pair: &ProviderModelPair, _api_key: &str) -> Result<CachedContext, String> {
    let mut model =
        get_model(pair.provider, pair.model).map_err(|error| format!("{BLOCKER}: {error}"))?;
    if let Some(api) = pair.api_override {
        model.api = api.to_owned();
    }

    let _tool = test_tool();
    let _headers = get_headers(pair);

    Err(format!(
        "{BLOCKER}; source behavior requires a live completeSimple tool-call round trip for {}",
        pair.label
    ))
}

fn available_contexts() -> Vec<CachedContext> {
    let mut contexts = Vec::new();

    for pair in PROVIDER_MODEL_PAIRS {
        let Some(api_key) = get_api_key(pair.provider) else {
            continue;
        };
        if !has_api_key(pair) {
            continue;
        }

        match generate_context(pair, &api_key) {
            Ok(context) if context.messages.len() >= 4 => contexts.push(context),
            Ok(_) => {}
            Err(error) => dump_failure_payload(pair.label, &error, None, &[]),
        }
    }

    contexts
}

#[test]
#[ignore = "live provider test skipped; compat catalog/provider dispatch, OAuth resolveApiKey, completeSimple, and real provider streams are request-capture blockers"]
fn should_have_at_least_2_fixtures_to_test_handoffs() {
    if !has_any_api_key() {
        return;
    }

    let contexts = available_contexts();

    assert!(
        contexts.len() >= 2,
        "expected at least 2 generated cross-provider fixtures"
    );
}

#[test]
#[ignore = "live provider test skipped; handoff requests require generated live fixtures and completeSimple provider calls"]
fn should_handle_cross_provider_handoffs_for_each_target() {
    if !has_any_api_key() {
        return;
    }

    let contexts = available_contexts();
    if contexts.len() < 2 {
        return;
    }

    let contexts_by_label = contexts
        .iter()
        .map(|context| (context.label, context))
        .collect::<HashMap<_, _>>();
    let available_pairs = PROVIDER_MODEL_PAIRS
        .iter()
        .filter(|pair| contexts_by_label.contains_key(pair.label));
    let mut results = Vec::new();

    for target_pair in available_pairs {
        let Some(_api_key) = get_api_key(target_pair.provider) else {
            continue;
        };
        if !has_api_key(target_pair) {
            continue;
        }

        let other_messages = contexts
            .iter()
            .filter(|context| context.label != target_pair.label)
            .flat_map(|context| context.messages.iter().cloned())
            .collect::<Vec<_>>();

        if other_messages.is_empty() {
            continue;
        }

        let mut model = match get_model(target_pair.provider, target_pair.model) {
            Ok(model) => model,
            Err(error) => {
                results.push(HandoffResult {
                    target: target_pair.label,
                    success: false,
                    error: Some(format!("{BLOCKER}: {error}")),
                });
                continue;
            }
        };
        if let Some(api) = target_pair.api_override {
            model.api = api.to_owned();
        }
        let _headers = get_headers(target_pair);
        let _tool = test_tool();

        results.push(HandoffResult {
            target: target_pair.label,
            success: false,
            error: Some(format!(
                "{BLOCKER}; source behavior requires live completeSimple handoff request for {}",
                target_pair.label
            )),
        });
    }

    let failures = results
        .iter()
        .filter(|result| !result.success)
        .collect::<Vec<_>>();

    assert_eq!(failures.len(), 0, "failures: {failures:#?}");
}
