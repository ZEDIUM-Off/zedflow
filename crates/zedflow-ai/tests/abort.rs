//! Port of Pi `packages/ai/test/abort.test.ts`.
//!
//! The source test only exercises live providers. Keep these parity tests ignored until the
//! Rust provider transports and the `StreamOptions.signal` abort bridge are implemented.

use zedflow_ai::api::lazy::{AssistantMessage, StopReason};

const BLOCKER: &str = "requires live provider transports, credentials, and a Rust AbortSignal bridge for StreamOptions.signal";

#[derive(Debug, Clone, Copy)]
struct ProviderAbortCase {
    provider: &'static str,
    model: &'static str,
    api_override: Option<&'static str>,
    options: &'static str,
}

const PROVIDER_ABORT_CASES: &[ProviderAbortCase] = &[
    ProviderAbortCase {
        provider: "google",
        model: "gemini-2.5-flash",
        api_override: None,
        options: "thinking.enabled=true",
    },
    ProviderAbortCase {
        provider: "openai",
        model: "gpt-4o-mini",
        api_override: Some("openai-completions"),
        options: "default",
    },
    ProviderAbortCase {
        provider: "openai",
        model: "gpt-5-mini",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        api_override: None,
        options: "azureDeploymentName when resolved",
    },
    ProviderAbortCase {
        provider: "anthropic",
        model: "claude-opus-4-1-20250805",
        api_override: None,
        options: "thinkingEnabled=true, thinkingBudgetTokens=2048",
    },
    ProviderAbortCase {
        provider: "mistral",
        model: "devstral-medium-latest",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        api_override: None,
        options: "reasoningEffort=high",
    },
    ProviderAbortCase {
        provider: "minimax",
        model: "MiniMax-M2.7",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        api_override: None,
        options: "default",
    },
    ProviderAbortCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        api_override: None,
        options: "apiKey from openai-codex OAuth token",
    },
    ProviderAbortCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        api_override: None,
        options: "reasoning=medium for mid-stream abort; default for immediate abort",
    },
];

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_abort_mid_stream_live_parity() {
    for case in PROVIDER_ABORT_CASES {
        let (aborted, follow_up) = run_live_mid_stream_abort(*case);

        assert_eq!(aborted.stop_reason, StopReason::Aborted);
        assert!(!aborted.content.is_empty());
        assert_eq!(follow_up.stop_reason, StopReason::Stop);
        assert!(!follow_up.content.is_empty());
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_immediate_abort_live_parity() {
    for case in PROVIDER_ABORT_CASES {
        let response = run_live_immediate_abort(*case);

        assert_eq!(response.stop_reason, StopReason::Aborted);
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn bedrock_abort_then_new_message_live_parity() {
    let bedrock = *PROVIDER_ABORT_CASES
        .iter()
        .find(|case| case.provider == "amazon-bedrock")
        .expect("Bedrock source case should stay represented");
    let (aborted, follow_up) = run_live_abort_then_new_message(bedrock);

    assert_eq!(aborted.stop_reason, StopReason::Aborted);
    assert!(aborted.content.is_empty());
    assert_eq!(follow_up.stop_reason, StopReason::Stop);
    assert!(!follow_up.content.is_empty());
}

fn run_live_mid_stream_abort(case: ProviderAbortCase) -> (AssistantMessage, AssistantMessage) {
    panic!("{BLOCKER}: {case:?}")
}

fn run_live_immediate_abort(case: ProviderAbortCase) -> AssistantMessage {
    panic!("{BLOCKER}: {case:?}")
}

fn run_live_abort_then_new_message(
    case: ProviderAbortCase,
) -> (AssistantMessage, AssistantMessage) {
    panic!("{BLOCKER}: {case:?}")
}
