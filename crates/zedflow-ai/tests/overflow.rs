use zedflow_ai::types::{AssistantMessage, AssistantMessageRole, StopReason, Usage, UsageCost};
use zedflow_ai::utils::overflow::is_context_overflow;

fn message(
    reason: StopReason,
    error: Option<&str>,
    input: u64,
    cache: u64,
    output: u64,
) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![],
        api: "openai-completions".into(),
        provider: "ollama".into(),
        model: "qwen3.5:35b".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input,
            output,
            cache_read: cache,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: input + cache + output,
            cost: UsageCost::default(),
        },
        stop_reason: reason,
        error_message: error.map(str::to_owned),
        timestamp: 0,
    }
}

#[test]
fn matches_pi_overflow_positive_and_negative_matrix() {
    let positives = [
        "400 `prompt too long; exceeded max context length by 100918 tokens`",
        "400 The input (516368 tokens) is longer than the model's context length (262144 tokens).",
        "Error: 503 litellm.ServiceUnavailableError: OpenAIException - Requested token count exceeds the model's maximum context length of 131072 tokens.",
        "Error: 400 Input length (265330) exceeds model's maximum context length (262144).",
        "Provider returned error: Input length 131393 exceeds the maximum allowed input length of 131040 tokens.",
        "400 Prompt has 256468 tokens, but the configured context size is 256000 tokens",
        "Prompt has 5,958,968 tokens, but the configured context size is 256,000 tokens",
    ];
    for error in positives {
        assert!(
            is_context_overflow(
                &message(StopReason::Error, Some(error), 0, 0, 0),
                Some(262_144)
            ),
            "{error}"
        );
    }

    let negatives = [
        "500 `model runner crashed unexpectedly`",
        "Throttling error: Too many tokens, please wait before trying again.",
        "Service unavailable: The service is temporarily unavailable.",
        "Rate limit exceeded, please retry after 30 seconds.",
        "Too many requests. Please slow down.",
    ];
    for error in negatives {
        assert!(
            !is_context_overflow(
                &message(StopReason::Error, Some(error), 0, 0, 0),
                Some(200_000)
            ),
            "{error}"
        );
    }

    assert!(is_context_overflow(
        &message(StopReason::Length, None, 58, 1_048_512, 0),
        Some(1_048_576)
    ));
    assert!(!is_context_overflow(
        &message(StopReason::Length, None, 1_000, 0, 4_096),
        Some(200_000)
    ));
    assert!(!is_context_overflow(
        &message(StopReason::Length, None, 100, 0, 0),
        Some(200_000)
    ));
}
