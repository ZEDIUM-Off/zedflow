use zedflow_ai::types::{AssistantMessage, AssistantMessageRole, StopReason, Usage, UsageCost};
use zedflow_ai::utils::retry::is_retryable_assistant_error;

fn message(reason: StopReason, error: Option<&str>) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![],
        api: "openai-completions".into(),
        provider: "faux".into(),
        model: "faux".into(),
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
            cost: UsageCost::default(),
        },
        stop_reason: reason,
        error_message: error.map(str::to_owned),
        timestamp: 0,
    }
}

#[test]
fn matches_pi_provider_retry_classification() {
    let retryable = [
        "An error occurred while processing your request. You can retry your request, or contact us through our help center at help.openai.com if the error persists.",
        r#"{"message":"The system encountered an unexpected error during processing. Try your request again."}"#,
        "overloaded_error",
        "524 status code (no body)",
    ];
    for error in retryable {
        assert!(
            is_retryable_assistant_error(&message(StopReason::Error, Some(error))),
            "{error}"
        );
    }

    assert!(!is_retryable_assistant_error(&message(
        StopReason::Error,
        Some("429 quota exceeded")
    )));
    assert!(!is_retryable_assistant_error(&message(
        StopReason::Stop,
        None
    )));
}
