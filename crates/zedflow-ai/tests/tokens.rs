//! Port of Pi `packages/ai/test/tokens.test.ts`.
//!
//! The source test aborts live provider streams after receiving at least 1000 text/thinking
//! characters, then asserts provider-specific token usage. P1.T2 forbids live provider calls, and
//! Rust provider streaming plus the abort signal bridge are still port placeholders, so this parity
//! test is ignored until those blockers are removed.

use zedflow_ai::types::{StopReason, Usage};

const BLOCKER: &str = "requires live provider transports, credentials/OAuth token resolution, completed compat::get_model/get_models, and a Rust StreamOptions.signal abort bridge";
const SOURCE_PROMPT: &str = "Write a long poem with 20 stanzas about the beauty of nature.";
const SOURCE_SYSTEM_PROMPT: &str = "You are a helpful assistant.";
const ABORT_AFTER_CHARS: usize = 1000;

#[derive(Debug, Clone, Copy)]
struct TokenAbortCase {
    provider: &'static str,
    model: &'static str,
    api: &'static str,
    options: &'static str,
    credential_gate: &'static str,
    cost_input: f64,
    upstream_skip_reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq)]
struct TokenAbortMessage {
    stop_reason: StopReason,
    usage: Usage,
}

const TOKEN_ABORT_CASES: &[TokenAbortCase] = &[
    TokenAbortCase {
        provider: "google",
        model: "gemini-2.5-flash",
        api: "google-generative-ai",
        options: "thinking.enabled=true",
        credential_gate: "GEMINI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "openai",
        model: "gpt-4o-mini",
        api: "openai-completions",
        options: "api override from openai model",
        credential_gate: "OPENAI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "openai",
        model: "gpt-5.4-mini",
        api: "openai-responses",
        options: "reasoningEffort=low",
        credential_gate: "OPENAI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        api: "azure-openai-responses",
        options: "azureDeploymentName when resolved",
        credential_gate: "Azure OpenAI endpoint/deployment credentials",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "anthropic",
        model: "claude-sonnet-4-6",
        api: "anthropic-messages",
        options: "default",
        credential_gate: "ANTHROPIC_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "xai",
        model: "grok-3-fast",
        api: "openai-completions",
        options: "default",
        credential_gate: "XAI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "groq",
        model: "openai/gpt-oss-20b",
        api: "openai-completions",
        options: "default",
        credential_gate: "GROQ_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "cerebras",
        model: "first available of gpt-oss-120b, zai-glm-4.7, llama3.1-8b, or catalog[0]",
        api: "openai-completions",
        options: "default",
        credential_gate: "CEREBRAS_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        api: "openai-completions",
        options: "default",
        credential_gate: "Cloudflare Workers AI credentials",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        api: "openai-completions",
        options: "default",
        credential_gate: "Cloudflare AI Gateway credentials",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        api: "openai-completions",
        options: "default",
        credential_gate: "HF_TOKEN",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        api: "openai-completions",
        options: "default",
        credential_gate: "TOGETHER_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "zai",
        model: "glm-4.5-air",
        api: "openai-completions",
        options: "default",
        credential_gate: "ZAI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "mistral",
        model: "devstral-medium-latest",
        api: "mistral-conversations",
        options: "default",
        credential_gate: "MISTRAL_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "minimax",
        model: "MiniMax-M2.7",
        api: "openai-completions",
        options: "default",
        credential_gate: "MINIMAX_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "kimi-coding",
        model: "kimi-for-coding",
        api: "openai-completions",
        options: "default",
        credential_gate: "KIMI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        api: "openai-completions",
        options: "default",
        credential_gate: "AI_GATEWAY_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "default",
        credential_gate: "XIAOMI_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: Some(
            "upstream Xiaomi stream only reports usage at message_stop, so abort loses token counts",
        ),
    },
    TokenAbortCase {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "default",
        credential_gate: "XIAOMI_TOKEN_PLAN_CN_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: Some(
            "upstream Xiaomi Token Plan stream only reports usage at message_stop, so abort loses token counts",
        ),
    },
    TokenAbortCase {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "default",
        credential_gate: "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: Some(
            "upstream Xiaomi Token Plan stream only reports usage at message_stop, so abort loses token counts",
        ),
    },
    TokenAbortCase {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        api: "anthropic-messages",
        options: "default",
        credential_gate: "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
        cost_input: 1.0,
        upstream_skip_reason: Some(
            "upstream Xiaomi Token Plan stream only reports usage at message_stop, so abort loses token counts",
        ),
    },
    TokenAbortCase {
        provider: "anthropic",
        model: "claude-sonnet-4-6",
        api: "anthropic-messages",
        options: "apiKey from anthropic OAuth token",
        credential_gate: "~/.pi/agent/oauth.json anthropic token",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "github-copilot",
        model: "claude-haiku-4.5",
        api: "anthropic-messages",
        options: "apiKey from github-copilot OAuth token",
        credential_gate: "~/.pi/agent/oauth.json github-copilot token",
        cost_input: 0.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        api: "anthropic-messages",
        options: "apiKey from github-copilot OAuth token",
        credential_gate: "~/.pi/agent/oauth.json github-copilot token",
        cost_input: 0.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        api: "openai-codex-responses",
        options: "apiKey from openai-codex OAuth token",
        credential_gate: "~/.pi/agent/oauth.json openai-codex token",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
    TokenAbortCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        api: "bedrock-converse-stream",
        options: "default",
        credential_gate: "AWS Bedrock credentials",
        cost_input: 1.0,
        upstream_skip_reason: None,
    },
];

#[test]
#[ignore = "live provider parity test skipped; see BLOCKER"]
fn provider_token_stats_on_abort_live_parity() {
    for case in TOKEN_ABORT_CASES {
        if case.upstream_skip_reason.is_some() {
            continue;
        }

        let message = run_live_tokens_on_abort(*case);
        assert_tokens_on_abort(*case, &message);
    }
}

#[test]
#[ignore = "source test has explicit it.skip Xiaomi cases; see upstream_skip_reason"]
fn xiaomi_token_stats_on_abort_remain_upstream_blocked() {
    for case in TOKEN_ABORT_CASES
        .iter()
        .filter(|case| case.upstream_skip_reason.is_some())
    {
        let message = run_live_tokens_on_abort(*case);
        assert_tokens_on_abort(*case, &message);
    }
}

fn run_live_tokens_on_abort(case: TokenAbortCase) -> TokenAbortMessage {
    let _source_fixture = (
        SOURCE_SYSTEM_PROMPT,
        SOURCE_PROMPT,
        ABORT_AFTER_CHARS,
        case.provider,
        case.model,
        case.api,
        case.options,
        case.credential_gate,
    );

    panic!("{BLOCKER}: {case:?}");
}

fn assert_tokens_on_abort(case: TokenAbortCase, message: &TokenAbortMessage) {
    assert_eq!(message.stop_reason, StopReason::Aborted);

    if matches!(
        case.api,
        "openai-completions"
            | "mistral-conversations"
            | "openai-responses"
            | "azure-openai-responses"
            | "openai-codex-responses"
    ) || matches!(
        case.provider,
        "zai" | "amazon-bedrock" | "vercel-ai-gateway" | "minimax"
    ) {
        assert_eq!(message.usage.input, 0);
        assert_eq!(message.usage.output, 0);
    } else if case.provider == "kimi-coding" {
        assert!(message.usage.input > 0);
        assert_eq!(message.usage.output, 0);
    } else {
        assert!(message.usage.input > 0);
        assert!(message.usage.output > 0);

        if case.cost_input > 0.0 {
            assert!(message.usage.cost.input > 0.0);
            assert!(message.usage.cost.total > 0.0);
        }
    }
}
