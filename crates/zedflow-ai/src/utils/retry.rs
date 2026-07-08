//! Retry classification helpers ported from Pi's `packages/ai/src/utils/retry.ts`.

use std::sync::LazyLock;

use regex::{Regex, RegexBuilder};

use crate::types::{AssistantMessage, StopReason};

static NON_RETRYABLE_PROVIDER_LIMIT_ERROR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    build_provider_error_pattern(&[
        // OpenCode Go/free-tier limits returned as 429 JSON error types by OpenCode's
        // Zen API. These are subscription/account limits, not transient throttles.
        "GoUsageLimitError",
        "FreeUsageLimitError",
        // OpenCode Go subscription-limit text asks users to enable available-balance
        // usage after rolling/weekly/monthly limits are reached.
        "Monthly usage limit reached",
        "available balance",
        // Generic quota/budget/billing exhaustion. `insufficient_quota` is OpenAI's
        // quota/billing error code; the other strings cover common gateway wording.
        "insufficient_quota",
        "out of budget",
        "quota exceeded",
        "billing",
    ])
});

static RETRYABLE_PROVIDER_ERROR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    build_provider_error_pattern(&[
        // Generic provider load, HTTP status, and server-side transient failures.
        "overloaded",
        "rate.?limit",
        "too many requests",
        "429",
        "500",
        "502",
        "503",
        "504",
        "524",
        "service.?unavailable",
        "server.?error",
        "internal.?error",
        // Wrapper/provider text for transient upstream failures, including OpenRouter
        // "Provider returned error" responses (#2264).
        "provider.?returned.?error",
        // Network, proxy, and fetch transport failures. This includes OpenAI Codex
        // raw-fetch failures such as "upstream connect", "connection refused", and
        // "reset before headers" (#733), plus OpenRouter connection drops (#3317).
        "network.?error",
        "connection.?error",
        "connection.?refused",
        "connection.?lost",
        "other side closed",
        "fetch failed",
        "upstream.?connect",
        "reset before headers",
        "socket hang up",
        "timed? out",
        "timeout",
        "terminated",
        // WebSocket transports can report close/error text instead of HTTP/fetch text.
        "websocket.?closed",
        "websocket.?error",
        // Premature stream endings from SDKs and transports. Anthropic can throw
        // "stream ended without ..." and "Anthropic stream ended before message_stop"
        // (#4433); Bedrock/Smithy can throw an HTTP/2 no-response error (#3594).
        "ended without",
        "stream ended before message_stop",
        "http2 request did not get a response",
        // Provider-requested retry delay cap failures should flow through the outer
        // retry policy so callers can surface/abort the backoff (#1123).
        "retry delay",
        // Explicit retry guidance emitted mid-stream by OpenAI Responses and Bedrock
        // stream exceptions (#6019).
        "you can retry your request",
        "try your request again",
        "please retry your request",
    ])
});

fn build_provider_error_pattern(patterns: &[&str]) -> Regex {
    RegexBuilder::new(&patterns.join("|"))
        .case_insensitive(true)
        .build()
        .expect("Pi retry provider error pattern is valid")
}

/// Classifies whether a failed assistant message looks like a transient provider
/// or transport error.
///
/// This mirrors Pi's `isRetryableAssistantError`: it only returns true for
/// assistant messages stopped with [`StopReason::Error`], with a non-empty error
/// message matching Pi's retryable provider/transport patterns and not matching
/// Pi's non-retryable account/quota limit patterns.
#[must_use]
pub fn is_retryable_assistant_error(message: &AssistantMessage) -> bool {
    if message.stop_reason != StopReason::Error {
        return false;
    }

    let Some(error_message) = message
        .error_message
        .as_deref()
        .filter(|text| !text.is_empty())
    else {
        return false;
    };

    if NON_RETRYABLE_PROVIDER_LIMIT_ERROR_PATTERN.is_match(error_message) {
        return false;
    }

    RETRYABLE_PROVIDER_ERROR_PATTERN.is_match(error_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AssistantMessageRole, Usage, UsageCost};

    fn assistant_error(stop_reason: StopReason, error_message: Option<&str>) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: Vec::new(),
            api: "openai-responses".to_string(),
            provider: "openai".to_string(),
            model: "test-model".to_string(),
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
            stop_reason,
            error_message: error_message.map(str::to_string),
            timestamp: 0,
        }
    }

    const OPENAI_EXPLICIT_RETRY_MESSAGE: &str = "An error occurred while processing your request. You can retry your request, or contact us through our help center at help.openai.com if the error persists. Please include the request ID req_******** in your message.";
    const BEDROCK_EXPLICIT_RETRY_MESSAGE: &str = r#"{"message":"The system encountered an unexpected error during processing. Try your request again."}"#;

    #[test]
    fn matches_explicit_provider_retry_guidance() {
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some(OPENAI_EXPLICIT_RETRY_MESSAGE)
        )));
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some(BEDROCK_EXPLICIT_RETRY_MESSAGE)
        )));
    }

    #[test]
    fn keeps_provider_limit_errors_non_retryable() {
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("429 quota exceeded")
        )));
    }

    #[test]
    fn classifies_assistant_error_messages() {
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("overloaded_error")
        )));
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("524 status code (no body)")
        )));
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Stop,
            None
        )));
    }

    #[test]
    fn rejects_non_error_or_missing_error_messages() {
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Stop,
            Some("rate limit")
        )));
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            None
        )));
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("")
        )));
    }

    #[test]
    fn retries_transient_provider_and_transport_errors_case_insensitively() {
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("Provider Returned Error: SERVICE unavailable")
        )));
        assert!(is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("upstream connect reset before headers")
        )));
    }

    #[test]
    fn rejects_account_quota_limits_before_retryable_matches() {
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("GoUsageLimitError: 429 monthly cap")
        )));
        assert!(!is_retryable_assistant_error(&assistant_error(
            StopReason::Error,
            Some("insufficient_quota: 429")
        )));
    }
}
