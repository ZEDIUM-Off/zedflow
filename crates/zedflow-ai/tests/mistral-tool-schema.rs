use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::api::mistral_conversations_lazy::mistral_conversations_api;
use zedflow_ai::types::{Context, Model, ModelCost, StopReason, StreamOptions, Tool};

#[test]
fn serializes_the_exact_nested_tool_schema_without_weakening() {
    let model = Model {
        id: "devstral-medium-latest".into(),
        name: "Devstral Medium".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "http://127.0.0.1:9".into(),
        cost: ModelCost::default(),
        ..Model::default()
    };
    let parameters = json!({
        "type": "object",
        "properties": {
            "nested": {
                "type": "object",
                "properties": { "value": { "type": "string" } },
                "required": ["value"],
                "additionalProperties": false
            }
        },
        "required": ["nested"],
        "additionalProperties": false
    });
    let context = Context {
        tools: Some(vec![Tool {
            name: "inspect_schema".into(),
            description: "Inspect the schema".into(),
            parameters: parameters.clone(),
        }]),
        ..Context::default()
    };
    let captured = Arc::new(Mutex::new(None));
    let hook_capture = Arc::clone(&captured);
    let options = StreamOptions {
        api_key: Some("fake-key".into()),
        on_payload: Some(Arc::new(move |payload, _model| {
            let hook_capture = Arc::clone(&hook_capture);
            Box::pin(async move {
                *hook_capture.lock().expect("capture lock") = Some(payload.clone());
                Ok(Some(payload))
            })
        })),
        ..StreamOptions::default()
    };

    let response =
        block_on((mistral_conversations_api().stream)(&model, &context, Some(&options)).result());
    assert_eq!(response.stop_reason, StopReason::Error);
    assert!(
        !response
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Input validation failed")
    );

    let payload = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("payload captured before deterministic request failure");
    let tools = payload["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "inspect_schema");
    assert_eq!(tools[0]["function"]["description"], "Inspect the schema");
    assert_eq!(tools[0]["function"]["parameters"], parameters);
    assert_eq!(tools[0]["function"]["strict"], false);
    assert!(matches!(
        tools[0]["function"]["parameters"],
        Value::Object(_)
    ));
}
