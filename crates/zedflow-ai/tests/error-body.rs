use serde_json::{Value, json};
use zedflow_ai::utils::error_body::{
    MAX_PROVIDER_ERROR_BODY_CHARS, ProviderErrorInput, SdkErrorShape, format_provider_error,
    normalize_provider_error,
};

#[test]
fn ports_pi_error_body_vectors_exactly() {
    let mistral = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "Mistral request failed".into(),
        status_code: Some(403.0),
        body: Some(Value::String(
            r#"{"error":"blocked by gateway WAF"}"#.into(),
        )),
        ..SdkErrorShape::default()
    }));
    assert_eq!(mistral.status, Some(403.0));
    assert_eq!(
        mistral.body.as_deref(),
        Some(r#"{"error":"blocked by gateway WAF"}"#)
    );
    assert!(!mistral.message_carries_body);

    let openai = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "403 status code (no body)".into(),
        status: Some(403.0),
        error: Some(json!({ "error": "blocked by gateway WAF" })),
        ..SdkErrorShape::default()
    }));
    assert_eq!(
        openai.body.as_deref(),
        Some(r#"{"error":"blocked by gateway WAF"}"#)
    );
    assert_eq!(
        format_provider_error(&openai, None),
        r#"403: {"error":"blocked by gateway WAF"}"#
    );
    assert_eq!(
        format_provider_error(&openai, Some("OpenAI API error")),
        r#"OpenAI API error (403): {"error":"blocked by gateway WAF"}"#
    );

    let google_body = json!({ "error": { "code": 403, "message": "Permission denied" } });
    let google_message = google_body.to_string();
    let google = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: google_message.clone(),
        status: Some(403.0),
        ..SdkErrorShape::default()
    }));
    assert!(google.message_carries_body);
    assert_eq!(google.message, google_message);
    assert_eq!(
        format_provider_error(&google, Some("OpenAI API error")),
        format!("OpenAI API error (403): {}", google.message)
    );

    let bedrock = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "UnknownError".into(),
        metadata_http_status_code: Some(403.0),
        response_status_code: Some(403.0),
        response_body: Some(Value::String(
            r#"{"message":"blocked by gateway WAF"}"#.into(),
        )),
        ..SdkErrorShape::default()
    }));
    assert_eq!(bedrock.status, Some(403.0));
    assert_eq!(
        bedrock.body.as_deref(),
        Some(r#"{"message":"blocked by gateway WAF"}"#)
    );
    assert!(!bedrock.message_carries_body);

    let non_error = normalize_provider_error(&ProviderErrorInput::NonErrorJson(json!({
        "reason": "boom"
    })));
    assert_eq!(non_error.status, None);
    assert_eq!(non_error.body, None);
    assert_eq!(non_error.message, r#"{"reason":"boom"}"#);
    assert_eq!(format_provider_error(&non_error, None), non_error.message);

    let empty = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "403 status code (no body)".into(),
        status: Some(403.0),
        error: Some(json!({})),
        ..SdkErrorShape::default()
    }));
    assert_eq!(empty.body, None);
    assert!(empty.message_carries_body);

    let long_body = "x".repeat(MAX_PROVIDER_ERROR_BODY_CHARS + 50);
    let truncated = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "failed".into(),
        status_code: Some(500.0),
        body: Some(Value::String(long_body.clone())),
        ..SdkErrorShape::default()
    }));
    let truncated_body = truncated.body.expect("truncated body");
    assert!(truncated_body.contains("... [truncated 50 chars]"));
    assert!(truncated_body.len() < long_body.len());

    let carried = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "500: upstream exploded".into(),
        status_code: Some(500.0),
        body: Some(Value::String("upstream exploded".into())),
        ..SdkErrorShape::default()
    }));
    assert!(carried.message_carries_body);
}
