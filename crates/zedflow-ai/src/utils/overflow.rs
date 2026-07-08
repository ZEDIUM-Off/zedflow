//! Context-overflow detection ported from Pi's `packages/ai/src/utils/overflow.ts`.

use std::sync::LazyLock;

use regex::Regex;

use crate::types::{AssistantMessage, StopReason};

static OVERFLOW_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    compile_patterns(&[
        r"(?i)prompt is too long",
        r"(?i)request_too_large",
        r"(?i)input is too long for requested model",
        r"(?i)exceeds the context window",
        r"(?i)exceeds (?:the )?(?:model'?s )?maximum context length(?: of [0-9,]+ tokens?|\s*\([0-9,]+\))",
        r"(?i)input token count.*exceeds the maximum",
        r"(?i)maximum prompt length is [0-9]+",
        r"(?i)reduce the length of the messages",
        r"(?i)maximum context length is [0-9]+ tokens",
        r"(?i)exceeds (?:the )?maximum allowed input length of [0-9,]+ tokens?",
        r"(?i)input \([0-9]+ tokens\) is longer than the model'?s context length \([0-9]+ tokens\)",
        r"(?i)exceeds the limit of [0-9]+",
        r"(?i)exceeds the available context size",
        r"(?i)greater than the context length",
        r"(?i)context window exceeds limit",
        r"(?i)exceeded model token limit",
        r"(?i)too large for model with [0-9]+ maximum context length",
        r"(?i)prompt has [0-9,]+ tokens?, but the configured context size is [0-9,]+ tokens?",
        r"(?i)model_context_window_exceeded",
        r"(?i)prompt too long; exceeded (?:max )?context length",
        r"(?i)context[_ ]length[_ ]exceeded",
        r"(?i)too many tokens",
        r"(?i)token limit exceeded",
        r"(?i)^4(?:00|13)\s*(?:status code)?\s*\(no body\)",
    ])
});

static NON_OVERFLOW_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    compile_patterns(&[
        r"(?i)^(Throttling error|Service unavailable):",
        r"(?i)rate limit",
        r"(?i)too many requests",
    ])
});

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|pattern| Regex::new(pattern).expect("ported overflow regex pattern is valid"))
        .collect()
}

/// Checks whether an assistant message represents a Pi context-overflow error.
///
/// This matches Pi's error-message patterns, silent z.ai-style overflows when a
/// context window is supplied, and Xiaomi MiMo-style length stops that fill the
/// context window with no output.
#[must_use]
pub fn is_context_overflow(message: &AssistantMessage, context_window: Option<u64>) -> bool {
    if message.stop_reason == StopReason::Error
        && let Some(error_message) = &message.error_message
    {
        let is_non_overflow = NON_OVERFLOW_PATTERNS
            .iter()
            .any(|pattern| pattern.is_match(error_message));
        if !is_non_overflow
            && OVERFLOW_PATTERNS
                .iter()
                .any(|pattern| pattern.is_match(error_message))
        {
            return true;
        }
    }

    let Some(context_window) = context_window.filter(|value| *value > 0) else {
        return false;
    };

    let input_tokens = message.usage.input.saturating_add(message.usage.cache_read);

    if message.stop_reason == StopReason::Stop && input_tokens > context_window {
        return true;
    }

    message.stop_reason == StopReason::Length
        && message.usage.output == 0
        && u128::from(input_tokens) * 100 >= u128::from(context_window) * 99
}

/// Returns copies of the Pi context-overflow regex patterns for tests.
#[must_use]
pub fn get_overflow_patterns() -> Vec<Regex> {
    OVERFLOW_PATTERNS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessageRole, Usage, UsageCost};

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
            provider: "openai".to_string(),
            model: "model".to_string(),
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

    fn create_error_message(error_message: &str) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: Vec::new(),
            api: "openai-completions".to_string(),
            provider: "ollama".to_string(),
            model: "qwen3.5:35b".to_string(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage {
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                cache_write_1h: None,
                reasoning: None,
                total_tokens: 0,
                cost: UsageCost {
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total: 0.0,
                },
            },
            stop_reason: StopReason::Error,
            error_message: Some(error_message.to_string()),
            timestamp: 0,
        }
    }

    fn create_length_stop_message(input: u64, cache_read: u64, output: u64) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: Vec::new(),
            api: "openai-completions".to_string(),
            provider: "xiaomi".to_string(),
            model: "mimo-v2.5-pro".to_string(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage {
                input,
                output,
                cache_read,
                cache_write: 0,
                cache_write_1h: None,
                reasoning: None,
                total_tokens: input.saturating_add(cache_read).saturating_add(output),
                cost: UsageCost {
                    input: 0.0,
                    output: 0.0,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total: 0.0,
                },
            },
            stop_reason: StopReason::Length,
            error_message: None,
            timestamp: 0,
        }
    }

    #[test]
    fn detects_explicit_ollama_prompt_too_long_errors() {
        let message = create_error_message(
            "400 `prompt too long; exceeded max context length by 100918 tokens`",
        );
        assert!(is_context_overflow(&message, Some(32768)));
    }

    #[test]
    fn detects_together_ai_context_length_errors() {
        let message = create_error_message(
            "400 The input (516368 tokens) is longer than the model's context length (262144 tokens).",
        );
        assert!(is_context_overflow(&message, Some(262144)));
    }

    #[test]
    fn detects_litellm_wrapped_openai_maximum_context_length_errors() {
        let message = create_error_message(
            "Error: 503 litellm.ServiceUnavailableError: litellm.MidStreamFallbackError: litellm.APIConnectionError: APIConnectionError: OpenAIException - Requested token count exceeds the model's maximum context length of 131072 tokens.",
        );
        assert!(is_context_overflow(&message, Some(131072)));
    }

    #[test]
    fn detects_openai_compatible_parenthesized_maximum_context_length_errors() {
        let message = create_error_message(
            "Error: 400 Input length (265330) exceeds model's maximum context length (262144).",
        );
        assert!(is_context_overflow(&message, Some(262144)));
    }

    #[test]
    fn detects_openrouter_poolside_maximum_allowed_input_length_errors() {
        let message = create_error_message(
            "Provider returned error: Input length 131393 exceeds the maximum allowed input length of 131040 tokens.",
        );
        assert!(is_context_overflow(&message, Some(131072)));
    }

    #[test]
    fn detects_ds4_configured_context_size_errors() {
        let message = create_error_message(
            "400 Prompt has 256468 tokens, but the configured context size is 256000 tokens",
        );
        assert!(is_context_overflow(&message, Some(256000)));

        let comma_message = create_error_message(
            "Prompt has 5,958,968 tokens, but the configured context size is 256,000 tokens",
        );
        assert!(is_context_overflow(&comma_message, Some(256000)));
    }

    #[test]
    fn does_not_treat_generic_non_overflow_ollama_errors_as_overflow() {
        let message = create_error_message("500 `model runner crashed unexpectedly`");
        assert!(!is_context_overflow(&message, Some(32768)));
    }

    #[test]
    fn does_not_treat_bedrock_throttling_too_many_tokens_as_overflow() {
        let message = create_error_message(
            "Throttling error: Too many tokens, please wait before trying again.",
        );
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn does_not_treat_bedrock_service_unavailable_as_overflow() {
        let message =
            create_error_message("Service unavailable: The service is temporarily unavailable.");
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn does_not_treat_generic_rate_limit_errors_as_overflow() {
        let message = create_error_message("Rate limit exceeded, please retry after 30 seconds.");
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn does_not_treat_http_429_style_errors_as_overflow() {
        let message = create_error_message("Too many requests. Please slow down.");
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn detects_xiaomi_style_overflow_length_stop_with_zero_output_and_filled_context() {
        let message = create_length_stop_message(58, 1048512, 0);
        assert!(is_context_overflow(&message, Some(1048576)));
    }

    #[test]
    fn does_not_treat_normal_length_stops_with_output_as_overflow() {
        let message = create_length_stop_message(1000, 0, 4096);
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn does_not_treat_length_stops_far_below_context_as_overflow() {
        let message = create_length_stop_message(100, 0, 0);
        assert!(!is_context_overflow(&message, Some(200000)));
    }

    #[test]
    fn detects_error_patterns_and_excludes_rate_limits() {
        let overflow = message(
            StopReason::Error,
            Some("Your input exceeds the context window of this model"),
            0,
            0,
        );
        assert!(is_context_overflow(&overflow, None));

        let throttled = message(
            StopReason::Error,
            Some("Throttling error: Too many tokens, please wait before trying again."),
            0,
            0,
        );
        assert!(!is_context_overflow(&throttled, None));
    }

    #[test]
    fn detects_silent_and_length_stop_overflow_with_context_window() {
        let silent = message(StopReason::Stop, None, 101, 1);
        assert!(is_context_overflow(&silent, Some(100)));

        let length_stop = message(StopReason::Length, None, 99, 0);
        assert!(is_context_overflow(&length_stop, Some(100)));
    }
}
