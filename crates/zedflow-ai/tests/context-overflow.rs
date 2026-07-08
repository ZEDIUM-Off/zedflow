//! Port of Pi's `packages/ai/test/context-overflow.test.ts`.
//!
//! The original test drives live providers through `complete(getModel(...))`.
//! P1.T2 forbids live provider/network/local-LLM calls, so the provider matrix
//! is represented with deterministic assistant messages plus an ignored live
//! parity marker.

use regex::Regex;
use zedflow_ai::types::{AssistantMessage, AssistantMessageRole, StopReason, Usage, UsageCost};
use zedflow_ai::utils::overflow::is_context_overflow;

const LIVE_PROVIDER_BLOCKER: &str = "Pi context-overflow.test.ts calls complete(getModel(...)) across live provider credentials, OAuth token helpers, and local Ollama/LM Studio/llama.cpp processes. P1.T2 forbids live provider/network/local-LLM calls, and zedflow-ai compat get_model/get_models/provider transports are still placeholders.";

fn message(
    stop_reason: StopReason,
    error_message: Option<&str>,
    input: u64,
    output: u64,
) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: "openai-responses".to_string(),
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input,
            output,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: input.saturating_add(output),
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason,
        error_message: error_message.map(str::to_string),
        timestamp: 0,
    }
}

#[test]
fn context_overflow_provider_error_patterns_match_pi_expectations() {
    struct Case {
        name: &'static str,
        error_message: &'static str,
        source_assertion: Option<&'static str>,
    }

    let cases = [
        Case {
            name: "anthropic api key",
            error_message: "prompt is too long: 213462 tokens > 200000 maximum",
            source_assertion: Some("(?i)prompt is too long"),
        },
        Case {
            name: "anthropic oauth",
            error_message: "prompt is too long: 213462 tokens > 200000 maximum",
            source_assertion: Some("(?i)prompt is too long"),
        },
        Case {
            name: "github copilot gemini",
            error_message: "prompt token count of 300000 exceeds the limit of 200000",
            source_assertion: Some("(?i)exceeds the limit of \\d+"),
        },
        Case {
            name: "github copilot claude",
            error_message: "prompt token count of 300000 exceeds the limit of 200000",
            source_assertion: Some("(?i)exceeds the limit of \\d+|input is too long"),
        },
        Case {
            name: "openai completions",
            error_message: "requested token count exceeds the model's maximum context length of 128000 tokens",
            source_assertion: Some("(?i)maximum context length"),
        },
        Case {
            name: "openai responses",
            error_message: "your input exceeds the context window of this model",
            source_assertion: Some("(?i)exceeds the context window"),
        },
        Case {
            name: "azure openai responses",
            error_message: "your input exceeds the context window maximum",
            source_assertion: Some("(?i)context|maximum"),
        },
        Case {
            name: "google gemini",
            error_message: "the input token count (1196265) exceeds the maximum number of tokens allowed (1048575)",
            source_assertion: Some("(?i)input token count.*exceeds the maximum"),
        },
        Case {
            name: "openai codex oauth",
            error_message: "token limit exceeded",
            source_assertion: None,
        },
        Case {
            name: "amazon bedrock",
            error_message: "Input is too long for requested model",
            source_assertion: None,
        },
        Case {
            name: "xai",
            error_message: "maximum prompt length is 131072 but the request contains 537812 tokens",
            source_assertion: Some("(?i)maximum prompt length is \\d+"),
        },
        Case {
            name: "groq",
            error_message: "please reduce the length of the messages or completion",
            source_assertion: Some("(?i)reduce the length of the messages"),
        },
        Case {
            name: "cerebras",
            error_message: "413 status code (no body)",
            source_assertion: Some("(?i)4(00|13|29).*\\(no body\\)"),
        },
        Case {
            name: "hugging face",
            error_message: "requested token count exceeds the model's maximum context length of 131072 tokens",
            source_assertion: None,
        },
        Case {
            name: "together ai",
            error_message: "the input (265330 tokens) is longer than the model's context length (262144 tokens)",
            source_assertion: None,
        },
        Case {
            name: "mistral",
            error_message: "prompt contains 300000 tokens and is too large for model with 128000 maximum context length",
            source_assertion: Some("(?i)too large for model with \\d+ maximum context length"),
        },
        Case {
            name: "minimax",
            error_message: "invalid params, context window exceeds limit",
            source_assertion: None,
        },
        Case {
            name: "kimi coding",
            error_message: "your request exceeded model token limit: 128000 (requested: 300000)",
            source_assertion: None,
        },
        Case {
            name: "vercel ai gateway",
            error_message: "the input token count (300000) exceeds the maximum number of tokens allowed (1048575)",
            source_assertion: None,
        },
        Case {
            name: "openrouter anthropic",
            error_message: "this endpoint's maximum context length is 200000 tokens; requested about 300000 tokens",
            source_assertion: Some("(?i)maximum context length is \\d+ tokens"),
        },
        Case {
            name: "openrouter deepseek",
            error_message: "this endpoint's maximum context length is 128000 tokens; requested about 300000 tokens",
            source_assertion: Some("(?i)maximum context length is \\d+ tokens"),
        },
        Case {
            name: "openrouter mistral",
            error_message: "this endpoint's maximum context length is 128000 tokens; requested about 300000 tokens",
            source_assertion: Some("(?i)maximum context length is \\d+ tokens"),
        },
        Case {
            name: "openrouter google",
            error_message: "this endpoint's maximum context length is 1048576 tokens; requested about 1200000 tokens",
            source_assertion: Some("(?i)maximum context length is \\d+ tokens"),
        },
        Case {
            name: "openrouter llama",
            error_message: "this endpoint's maximum context length is 10000000 tokens; requested about 10001000 tokens",
            source_assertion: Some("(?i)maximum context length is \\d+ tokens"),
        },
        Case {
            name: "lm studio",
            error_message: "tokens to keep from the initial prompt is greater than the context length",
            source_assertion: None,
        },
        Case {
            name: "llama.cpp",
            error_message: "the request exceeds the available context size, try increasing it",
            source_assertion: None,
        },
        Case {
            name: "ollama explicit error",
            error_message: "prompt too long; exceeded max context length by 1000 tokens",
            source_assertion: None,
        },
    ];

    for test_case in cases {
        if let Some(pattern) = test_case.source_assertion {
            let re = Regex::new(pattern).expect("ported source assertion regex should compile");
            assert!(
                re.is_match(test_case.error_message),
                "source assertion failed for {}",
                test_case.name
            );
        }

        let response = message(StopReason::Error, Some(test_case.error_message), 0, 0);
        assert_eq!(
            response.stop_reason,
            StopReason::Error,
            "{}",
            test_case.name
        );
        assert!(
            is_context_overflow(&response, Some(128_000)),
            "{}",
            test_case.name
        );
    }
}

#[test]
fn context_overflow_zai_and_xiaomi_special_cases_match_pi_expectations() {
    let zai_error = message(
        StopReason::Error,
        Some("model_context_window_exceeded"),
        0,
        0,
    );
    assert!(is_context_overflow(&zai_error, Some(128_000)));

    let zai_silent = message(StopReason::Stop, None, 128_001, 1);
    assert!(is_context_overflow(&zai_silent, Some(128_000)));

    let xiaomi_length = message(StopReason::Length, None, 99, 0);
    assert_eq!(xiaomi_length.stop_reason, StopReason::Length);
    assert_eq!(xiaomi_length.usage.output, 0);
    assert!(is_context_overflow(&xiaomi_length, Some(100)));
}

#[test]
fn context_overflow_ollama_silent_truncation_remains_not_detectable() {
    let silently_truncated = message(StopReason::Stop, None, 127_000, 1);
    assert!(!is_context_overflow(&silently_truncated, Some(128_000)));
}

#[test]
#[ignore = "live provider/network/local-LLM parity test skipped; see LIVE_PROVIDER_BLOCKER"]
fn context_overflow_live_provider_matrix_is_blocked() {
    panic!("{LIVE_PROVIDER_BLOCKER}");
}
