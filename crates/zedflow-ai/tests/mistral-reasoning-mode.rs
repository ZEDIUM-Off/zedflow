use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use serde_json::Value;
use zedflow_ai::api::mistral_conversations_lazy::mistral_conversations_api;
use zedflow_ai::types::{
    CacheRetention, Context, Model, ModelCost, SimpleStreamOptions, StopReason, StreamOptions,
    ThinkingLevel,
};

fn model(id: &str, reasoning: bool) -> Model {
    Model {
        id: id.into(),
        name: id.into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "http://127.0.0.1:9".into(),
        reasoning,
        cost: ModelCost::default(),
        ..Model::default()
    }
}

fn capture_payload(
    model: &Model,
    reasoning: Option<ThinkingLevel>,
    session_id: Option<&str>,
    cache_retention: Option<CacheRetention>,
) -> Value {
    let captured = Arc::new(Mutex::new(None));
    let hook_capture = Arc::clone(&captured);
    let options = SimpleStreamOptions {
        stream: StreamOptions {
            api_key: Some("fake-key".into()),
            session_id: session_id.map(str::to_owned),
            cache_retention,
            on_payload: Some(Arc::new(move |payload, _model| {
                let hook_capture = Arc::clone(&hook_capture);
                Box::pin(async move {
                    *hook_capture.lock().expect("capture lock") = Some(payload.clone());
                    Ok(Some(payload))
                })
            })),
            ..StreamOptions::default()
        },
        reasoning,
        thinking_budgets: None,
    };
    let response = block_on(
        (mistral_conversations_api().stream_simple)(model, &Context::default(), Some(&options))
            .result(),
    );
    assert_eq!(response.stop_reason, StopReason::Error);
    let payload = captured.lock().expect("capture lock").clone();
    payload.expect("payload captured before deterministic request failure")
}

#[test]
fn selects_exact_reasoning_mode_and_prompt_cache_fields() {
    let payload = capture_payload(
        &model("mistral-small-2603", true),
        Some(ThinkingLevel::Medium),
        None,
        None,
    );
    assert_eq!(payload["reasoningEffort"], "high");
    assert!(payload.get("promptMode").is_none());

    let payload = capture_payload(&model("mistral-small-2603", true), None, None, None);
    assert!(payload.get("reasoningEffort").is_none());
    assert!(payload.get("promptMode").is_none());

    let payload = capture_payload(
        &model("magistral-medium-latest", true),
        Some(ThinkingLevel::Medium),
        None,
        None,
    );
    assert_eq!(payload["promptMode"], "reasoning");
    assert!(payload.get("reasoningEffort").is_none());

    let payload = capture_payload(
        &model("mistral-medium-3.5", true),
        Some(ThinkingLevel::Medium),
        None,
        None,
    );
    assert_eq!(payload["reasoningEffort"], "high");
    assert!(payload.get("promptMode").is_none());

    let payload = capture_payload(&model("mistral-medium-3.5", true), None, None, None);
    assert!(payload.get("reasoningEffort").is_none());
    assert!(payload.get("promptMode").is_none());

    let payload = capture_payload(
        &model("mistral-large-latest", false),
        None,
        Some("session-123"),
        None,
    );
    assert_eq!(payload["promptCacheKey"], "session-123");

    let payload = capture_payload(
        &model("mistral-large-latest", false),
        None,
        Some("session-123"),
        Some(CacheRetention::None),
    );
    assert!(payload.get("promptCacheKey").is_none());
}
