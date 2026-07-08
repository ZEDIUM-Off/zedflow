//! Port of Pi `packages/ai/test/total-tokens.test.ts`.
//!
//! The source test makes live `complete` calls against every provider and asserts that reported
//! `totalTokens` equals `input + output + cacheRead + cacheWrite`. P1.T2 forbids live provider
//! calls, and Rust `compat::get_model` plus provider transports are still port placeholders, so the
//! live parity test is ignored until those blockers are removed.

use zedflow_ai::api::lazy::{StopReason, Usage};

const BLOCKER: &str = "requires live provider transports, credentials/OAuth token resolution, and completed compat::get_model/get_models";
const LONG_SYSTEM_PROMPT: &str = "long system prompt fixture from the TypeScript test, repeated enough to trigger provider prompt caching";
const FIRST_USER_PROMPT: &str = "What is 2 + 2? Reply with just the number.";
const SECOND_USER_PROMPT: &str = "What is 3 + 3? Reply with just the number.";

#[derive(Debug, Clone, Copy)]
struct TotalTokensCase {
    provider: &'static str,
    model: &'static str,
    api: &'static str,
    options: &'static str,
    credential_gate: &'static str,
    expects_cache_activity: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct TotalTokensResponse {
    stop_reason: StopReason,
    usage: Usage,
}

#[derive(Debug, Clone, PartialEq)]
struct TotalTokensRun {
    first: TotalTokensResponse,
    second: TotalTokensResponse,
}

const TOTAL_TOKENS_CASES: &[TotalTokensCase] = &[
    TotalTokensCase {
        provider: "anthropic",
        model: "claude-sonnet-4-5",
        api: "anthropic-messages",
        options: "apiKey from ANTHROPIC_API_KEY",
        credential_gate: "ANTHROPIC_API_KEY",
        expects_cache_activity: true,
    },
    TotalTokensCase {
        provider: "anthropic",
        model: "claude-sonnet-4-6",
        api: "anthropic-messages",
        options: "apiKey from anthropic OAuth token",
        credential_gate: "~/.pi/agent/oauth.json anthropic token",
        expects_cache_activity: true,
    },
    TotalTokensCase {
        provider: "openai",
        model: "gpt-4o-mini",
        api: "openai-completions",
        options: "api override from openai model",
        credential_gate: "OPENAI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openai",
        model: "gpt-4o",
        api: "openai-responses",
        options: "default",
        credential_gate: "OPENAI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        api: "azure-openai-responses",
        options: "azureDeploymentName when resolved",
        credential_gate: "Azure OpenAI endpoint/deployment credentials",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "google",
        model: "gemini-2.0-flash",
        api: "google-generative-ai",
        options: "default",
        credential_gate: "GEMINI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "xai",
        model: "grok-3-fast",
        api: "openai-completions",
        options: "apiKey from XAI_API_KEY",
        credential_gate: "XAI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "groq",
        model: "openai/gpt-oss-120b",
        api: "openai-completions",
        options: "apiKey from GROQ_API_KEY",
        credential_gate: "GROQ_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "cerebras",
        model: "gpt-oss-120b",
        api: "openai-completions",
        options: "apiKey from CEREBRAS_API_KEY",
        credential_gate: "CEREBRAS_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        api: "openai-completions",
        options: "apiKey from CLOUDFLARE_API_KEY",
        credential_gate: "Cloudflare Workers AI credentials",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        api: "openai-completions",
        options: "apiKey from CLOUDFLARE_API_KEY",
        credential_gate: "Cloudflare AI Gateway credentials",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        api: "openai-completions",
        options: "apiKey from HF_TOKEN",
        credential_gate: "HF_TOKEN",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        api: "openai-completions",
        options: "apiKey from TOGETHER_API_KEY, reasoningEffort=high",
        credential_gate: "TOGETHER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "zai",
        model: "glm-4.5-air",
        api: "openai-completions",
        options: "apiKey from ZAI_API_KEY",
        credential_gate: "ZAI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "mistral",
        model: "devstral-medium-latest",
        api: "mistral-conversations",
        options: "apiKey from MISTRAL_API_KEY",
        credential_gate: "MISTRAL_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "minimax",
        model: "MiniMax-M2.7",
        api: "openai-completions",
        options: "apiKey from MINIMAX_API_KEY",
        credential_gate: "MINIMAX_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "apiKey from XIAOMI_API_KEY",
        credential_gate: "XIAOMI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "apiKey from XIAOMI_TOKEN_PLAN_CN_API_KEY",
        credential_gate: "XIAOMI_TOKEN_PLAN_CN_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "apiKey from XIAOMI_TOKEN_PLAN_AMS_API_KEY",
        credential_gate: "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "apiKey from XIAOMI_TOKEN_PLAN_SGP_API_KEY",
        credential_gate: "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        api: "openai-completions",
        options: "apiKey from KIMI_API_KEY",
        credential_gate: "KIMI_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        api: "openai-completions",
        options: "apiKey from AI_GATEWAY_API_KEY",
        credential_gate: "AI_GATEWAY_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openrouter",
        model: "anthropic/claude-sonnet-4",
        api: "openai-completions",
        options: "apiKey from OPENROUTER_API_KEY",
        credential_gate: "OPENROUTER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openrouter",
        model: "deepseek/deepseek-chat",
        api: "openai-completions",
        options: "apiKey from OPENROUTER_API_KEY",
        credential_gate: "OPENROUTER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openrouter",
        model: "mistralai/mistral-small-3.2-24b-instruct",
        api: "openai-completions",
        options: "apiKey from OPENROUTER_API_KEY",
        credential_gate: "OPENROUTER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openrouter",
        model: "google/gemini-2.5-flash",
        api: "openai-completions",
        options: "apiKey from OPENROUTER_API_KEY",
        credential_gate: "OPENROUTER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openrouter",
        model: "deepseek/deepseek-chat",
        api: "openai-completions",
        options: "apiKey from OPENROUTER_API_KEY (duplicate source case)",
        credential_gate: "OPENROUTER_API_KEY",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "github-copilot",
        model: "claude-haiku-4.5",
        api: "anthropic-messages",
        options: "apiKey from github-copilot OAuth token",
        credential_gate: "~/.pi/agent/oauth.json github-copilot token",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        api: "anthropic-messages",
        options: "apiKey from github-copilot OAuth token",
        credential_gate: "~/.pi/agent/oauth.json github-copilot token",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        api: "bedrock-converse-stream",
        options: "default",
        credential_gate: "AWS Bedrock credentials",
        expects_cache_activity: false,
    },
    TotalTokensCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        api: "openai-codex-responses",
        options: "apiKey from openai-codex OAuth token",
        credential_gate: "~/.pi/agent/oauth.json openai-codex token",
        expects_cache_activity: false,
    },
];

#[test]
#[ignore = "live provider parity test skipped; see BLOCKER"]
fn total_tokens_live_provider_parity() {
    for case in TOTAL_TOKENS_CASES {
        let run = run_live_total_tokens_with_cache(*case);
        assert_total_tokens_with_cache(*case, &run);
    }
}

fn run_live_total_tokens_with_cache(case: TotalTokensCase) -> TotalTokensRun {
    let _source_fixture = (
        LONG_SYSTEM_PROMPT,
        FIRST_USER_PROMPT,
        SECOND_USER_PROMPT,
        case.provider,
        case.model,
        case.api,
        case.options,
        case.credential_gate,
    );

    panic!("{BLOCKER}: {case:?}");
}

fn assert_total_tokens_with_cache(case: TotalTokensCase, run: &TotalTokensRun) {
    assert_eq!(run.first.stop_reason, StopReason::Stop);
    assert_eq!(run.second.stop_reason, StopReason::Stop);
    assert_total_tokens_equals_components(&run.first.usage);
    assert_total_tokens_equals_components(&run.second.usage);

    if case.expects_cache_activity {
        let has_cache = run.second.usage.cache_read > 0
            || run.second.usage.cache_write > 0
            || run.first.usage.cache_write > 0;
        assert!(has_cache);
    }
}

fn assert_total_tokens_equals_components(usage: &Usage) {
    let computed = usage.input + usage.output + usage.cache_read + usage.cache_write;
    assert_eq!(usage.total_tokens, computed);
}
