//! Provider HTTP error body normalization ported from Pi's `packages/ai/src/utils/error-body.ts`.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use serde_json::Value;

/// Maximum number of UTF-16 code units Pi keeps from a provider error body before truncating.
pub const MAX_PROVIDER_ERROR_BODY_CHARS: usize = 4000;

/// Error-like input accepted by [`normalize_provider_error`].
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderErrorInput {
    /// A JavaScript `Error`-like SDK object with Pi's probed fields.
    Error(SdkErrorShape),
    /// A non-`Error` thrown value representable as JSON.
    NonErrorJson(Value),
    /// A non-`Error` thrown value whose JavaScript `String(value)` fallback is already known.
    NonErrorString(String),
}

/// SDK error fields probed by Pi provider error normalization.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SdkErrorShape {
    /// JavaScript `error.message`.
    pub message: String,
    /// Mistral-style `statusCode` field.
    pub status_code: Option<f64>,
    /// OpenAI and `@google/genai`-style `status` field.
    pub status: Option<f64>,
    /// Mistral-style raw `body` field; only string values are used.
    pub body: Option<Value>,
    /// OpenAI SDK parsed `error` body.
    pub error: Option<Value>,
    /// AWS Bedrock `$metadata.httpStatusCode` field.
    pub metadata_http_status_code: Option<f64>,
    /// AWS Bedrock `$response.statusCode` field.
    pub response_status_code: Option<f64>,
    /// AWS Bedrock `$response.body` field.
    pub response_body: Option<Value>,
}

/// Provider HTTP error details normalized from SDK-specific error fields.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedProviderError {
    /// HTTP status code, when one could be extracted from the SDK error object.
    pub status: Option<f64>,
    /// Raw HTTP body reason, already trimmed and truncated to the cap.
    pub body: Option<String>,
    /// `error.message`, or [`safe_json_stringify`] for a non-`Error` throw.
    pub message: String,
    /// True when `message` already contains the body and providers should not double-print it.
    pub message_carries_body: bool,
}

/// HTTP error fields from Rust clients that expose status/body/headers directly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderHttpErrorParts {
    /// Error message from the client error.
    pub message: String,
    /// HTTP status code, when present.
    pub status: Option<u16>,
    /// HTTP response headers, when present.
    pub headers: HashMap<String, String>,
    /// Raw HTTP body, when present.
    pub body: Option<String>,
}

impl ProviderHttpErrorParts {
    /// Creates HTTP error parts with a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Self::default()
        }
    }

    /// Sets the HTTP status code.
    #[must_use]
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets the raw body text.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Sets response headers.
    #[must_use]
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }
}

/// Normalized Rust HTTP error data plus headers that Pi's JS SDK shape did not carry.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedProviderHttpError {
    /// Pi-compatible normalized error display fields.
    pub normalized: NormalizedProviderError,
    /// HTTP response headers and provider metadata, when present.
    pub headers: HashMap<String, String>,
}

/// Canonical provider service failure used behind provider-specific transports.
///
/// The public shape contains only Zedflow-owned data while the private source
/// retains the concrete HTTP/SDK error for Rust error-chain diagnostics.
#[derive(Debug)]
pub struct ProviderServiceError {
    /// Normalized status, message, body, and response metadata.
    pub http: NormalizedProviderHttpError,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl ProviderServiceError {
    /// Creates a service error from provider-owned HTTP parts.
    #[must_use]
    pub fn new(parts: ProviderHttpErrorParts) -> Self {
        Self {
            http: normalize_provider_http_error(parts),
            source: None,
        }
    }

    /// Creates a service error while retaining its concrete source.
    #[must_use]
    pub fn with_source(
        parts: ProviderHttpErrorParts,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            http: normalize_provider_http_error(parts),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for ProviderServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&format_provider_error(&self.http.normalized, None))
    }
}

impl Error for ProviderServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Normalizes Rust HTTP client errors while preserving status/body/header data.
#[must_use]
pub fn normalize_provider_http_error(parts: ProviderHttpErrorParts) -> NormalizedProviderHttpError {
    let body = parts.body.and_then(|body| {
        let trimmed = body.trim();
        (!trimmed.is_empty()).then(|| truncate_error_text(trimmed, MAX_PROVIDER_ERROR_BODY_CHARS))
    });
    let message_carries_body = body
        .as_deref()
        .is_none_or(|body| parts.message.contains(body));

    NormalizedProviderHttpError {
        normalized: NormalizedProviderError {
            status: parts.status.map(f64::from),
            body,
            message: parts.message,
            message_carries_body,
        },
        headers: parts.headers,
    }
}

/// Normalizes provider SDK HTTP error objects into Pi's shared display data.
#[must_use]
pub fn normalize_provider_error(error: &ProviderErrorInput) -> NormalizedProviderError {
    match error {
        ProviderErrorInput::Error(error) => {
            let status = extract_status(error);
            let body = extract_body(error);
            let message_carries_body = body
                .as_deref()
                .is_none_or(|body| error.message.contains(body));

            NormalizedProviderError {
                status,
                body,
                message: error.message.clone(),
                message_carries_body,
            }
        }
        ProviderErrorInput::NonErrorJson(value) => NormalizedProviderError {
            status: None,
            body: None,
            message: safe_json_stringify(value),
            message_carries_body: false,
        },
        ProviderErrorInput::NonErrorString(value) => NormalizedProviderError {
            status: None,
            body: None,
            message: value.clone(),
            message_carries_body: false,
        },
    }
}

/// Composes a provider display string from a normalized error and optional provider prefix.
#[must_use]
pub fn format_provider_error(norm: &NormalizedProviderError, prefix: Option<&str>) -> String {
    if norm.message_carries_body || norm.status.is_none() || norm.body.is_none() {
        return match (prefix, norm.status) {
            (Some(prefix), Some(status)) => {
                format!("{prefix} ({}): {}", format_status(status), norm.message)
            }
            _ => norm.message.clone(),
        };
    }

    match (prefix, norm.status, norm.body.as_deref()) {
        (Some(prefix), Some(status), Some(body)) => {
            format!("{prefix} ({}): {body}", format_status(status))
        }
        (None, Some(status), Some(body)) => format!("{}: {body}", format_status(status)),
        _ => norm.message.clone(),
    }
}

/// Truncates provider error text to Pi's UTF-16 code-unit cap.
#[must_use]
pub fn truncate_error_text(text: &str, max_chars: usize) -> String {
    let units = text.encode_utf16().collect::<Vec<_>>();
    if units.len() <= max_chars {
        return text.to_string();
    }

    let truncated = char::decode_utf16(units.iter().take(max_chars).copied())
        .map(|item| item.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect::<String>();
    format!(
        "{truncated}... [truncated {} chars]",
        units.len() - max_chars
    )
}

/// Serializes a JSON value the way Pi's `safeJsonStringify` handles JSON-compatible values.
#[must_use]
pub fn safe_json_stringify(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

fn extract_status(error: &SdkErrorShape) -> Option<f64> {
    error
        .status_code
        .or(error.status)
        .or(error.metadata_http_status_code)
        .or(error.response_status_code)
}

fn extract_body(error: &SdkErrorShape) -> Option<String> {
    let body_text = pick_body_text(error)?;
    let trimmed = body_text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(truncate_error_text(trimmed, MAX_PROVIDER_ERROR_BODY_CHARS))
}

fn pick_body_text(error: &SdkErrorShape) -> Option<String> {
    if let Some(Value::String(body)) = &error.body {
        return Some(body.clone());
    }
    if let Some(value) = error
        .error
        .as_ref()
        .filter(|value| is_non_empty_object(value))
    {
        return Some(safe_json_stringify(value));
    }
    match &error.response_body {
        Some(Value::String(body)) => Some(body.clone()),
        Some(value) if is_non_empty_object(value) => Some(safe_json_stringify(value)),
        _ => None,
    }
}

fn is_non_empty_object(value: &Value) -> bool {
    match value {
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => false,
    }
}

fn format_status(status: f64) -> String {
    if status.is_nan() {
        return "NaN".to_string();
    }
    if status == f64::INFINITY {
        return "Infinity".to_string();
    }
    if status == f64::NEG_INFINITY {
        return "-Infinity".to_string();
    }
    if status == 0.0 {
        return "0".to_string();
    }
    status.to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn extracts_status_and_body_from_mistral_shaped_error() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "Mistral request failed".to_string(),
            status_code: Some(403.0),
            body: Some(Value::String(
                r#"{"error":"blocked by gateway WAF"}"#.to_string(),
            )),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, Some(403.0));
        assert_eq!(
            norm.body.as_deref(),
            Some(r#"{"error":"blocked by gateway WAF"}"#)
        );
        assert!(!norm.message_carries_body);
    }

    #[test]
    fn reads_parsed_body_off_openai_api_error_when_message_is_opaque() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "403 status code (no body)".to_string(),
            status: Some(403.0),
            error: Some(json!({ "error": "blocked by gateway WAF" })),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, Some(403.0));
        assert_eq!(
            norm.body.as_deref(),
            Some(r#"{"error":"blocked by gateway WAF"}"#)
        );
        assert!(!norm.message_carries_body);
    }

    #[test]
    fn preserves_message_when_google_genai_already_folds_body_into_it() {
        let body = json!({ "error": { "code": 403, "message": "Permission denied" } });
        let message = body.to_string();
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: message.clone(),
            status: Some(403.0),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, Some(403.0));
        assert!(norm.message_carries_body);
        assert_eq!(norm.message, message);
    }

    #[test]
    fn extracts_status_and_body_from_bedrock_shaped_service_exception() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "UnknownError".to_string(),
            metadata_http_status_code: Some(403.0),
            response_status_code: Some(403.0),
            response_body: Some(Value::String(
                r#"{"message":"blocked by gateway WAF"}"#.to_string(),
            )),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, Some(403.0));
        assert_eq!(
            norm.body.as_deref(),
            Some(r#"{"message":"blocked by gateway WAF"}"#)
        );
        assert!(!norm.message_carries_body);
    }

    #[test]
    fn json_stringifies_non_error_thrown_value() {
        let error = ProviderErrorInput::NonErrorJson(json!({ "reason": "boom" }));

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, None);
        assert_eq!(norm.body, None);
        assert_eq!(norm.message, r#"{"reason":"boom"}"#);
        assert!(!norm.message_carries_body);
    }

    #[test]
    fn treats_empty_parsed_body_object_as_no_body() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "403 status code (no body)".to_string(),
            status: Some(403.0),
            error: Some(json!({})),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.body, None);
        assert!(norm.message_carries_body);
    }

    #[test]
    fn truncates_body_at_the_cap() {
        let long_body = "x".repeat(MAX_PROVIDER_ERROR_BODY_CHARS + 50);
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "failed".to_string(),
            status_code: Some(500.0),
            body: Some(Value::String(long_body.clone())),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);
        let body = norm.body.expect("truncated body should be present");

        assert!(body.contains("... [truncated 50 chars]"));
        assert!(body.len() < long_body.len());
    }

    #[test]
    fn sets_message_carries_body_when_message_already_contains_extracted_body() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "500: upstream exploded".to_string(),
            status_code: Some(500.0),
            body: Some(Value::String("upstream exploded".to_string())),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert!(norm.message_carries_body);
    }

    #[test]
    fn surfaces_status_and_body_without_a_prefix() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "403 status code (no body)".to_string(),
            status: Some(403.0),
            error: Some(json!({ "error": "blocked by gateway WAF" })),
            ..SdkErrorShape::default()
        });
        let norm = normalize_provider_error(&error);

        let formatted = format_provider_error(&norm, None);

        assert!(formatted.contains("403"));
        assert!(formatted.contains("blocked by gateway WAF"));
        assert_ne!(formatted, "403 status code (no body)");
    }

    #[test]
    fn applies_provider_prefix_with_status_and_body() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "403 status code (no body)".to_string(),
            status: Some(403.0),
            error: Some(json!({ "error": "blocked by gateway WAF" })),
            ..SdkErrorShape::default()
        });
        let norm = normalize_provider_error(&error);

        assert_eq!(
            format_provider_error(&norm, Some("OpenAI API error")),
            r#"OpenAI API error (403): {"error":"blocked by gateway WAF"}"#
        );
    }

    #[test]
    fn preserves_message_with_prefix_and_status_when_it_already_carries_body() {
        let body = json!({ "error": { "message": "Permission denied" } }).to_string();
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: body.clone(),
            status: Some(403.0),
            ..SdkErrorShape::default()
        });
        let norm = normalize_provider_error(&error);

        assert_eq!(
            format_provider_error(&norm, Some("OpenAI API error")),
            format!("OpenAI API error (403): {body}")
        );
    }

    #[test]
    fn returns_bare_message_for_non_error_value() {
        let error = ProviderErrorInput::NonErrorJson(json!({ "reason": "boom" }));
        let norm = normalize_provider_error(&error);

        assert_eq!(format_provider_error(&norm, None), r#"{"reason":"boom"}"#);
    }

    #[test]
    fn normalizes_status_and_body_in_sdk_order() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "403 status code (no body)".to_string(),
            status_code: None,
            status: Some(403.0),
            body: None,
            error: Some(json!({ "error": "blocked" })),
            metadata_http_status_code: Some(500.0),
            response_status_code: Some(502.0),
            response_body: Some(Value::String("fallback".to_string())),
        });

        let norm = normalize_provider_error(&error);

        assert_eq!(norm.status, Some(403.0));
        assert_eq!(norm.body.as_deref(), Some(r#"{"error":"blocked"}"#));
        assert!(!norm.message_carries_body);
        assert_eq!(
            format_provider_error(&norm, Some("OpenAI")),
            r#"OpenAI (403): {"error":"blocked"}"#
        );
    }

    #[test]
    fn leaves_messages_that_already_contain_body_unchanged() {
        let error = ProviderErrorInput::Error(SdkErrorShape {
            message: "provider said denied".to_string(),
            status: Some(403.0),
            body: Some(Value::String("denied".to_string())),
            ..SdkErrorShape::default()
        });

        let norm = normalize_provider_error(&error);

        assert!(norm.message_carries_body);
        assert_eq!(format_provider_error(&norm, None), "provider said denied");
    }

    #[test]
    fn normalizes_rust_http_errors_with_headers_and_truncated_body() {
        let body = "x".repeat(MAX_PROVIDER_ERROR_BODY_CHARS + 1);
        let error = normalize_provider_http_error(
            ProviderHttpErrorParts::new("request failed")
                .with_status(502)
                .with_body(body)
                .with_headers(HashMap::from([(
                    "x-request-id".to_string(),
                    "req-1".to_string(),
                )])),
        );

        assert_eq!(error.normalized.status, Some(502.0));
        assert_eq!(
            error.headers.get("x-request-id").map(String::as_str),
            Some("req-1")
        );
        assert!(
            error
                .normalized
                .body
                .as_deref()
                .is_some_and(|body| body.ends_with("... [truncated 1 chars]"))
        );
        assert!(!error.normalized.message_carries_body);
    }

    #[test]
    fn truncates_by_utf16_code_units_like_javascript() {
        assert_eq!(
            truncate_error_text("abcdef", 3),
            "abc... [truncated 3 chars]"
        );
        assert_eq!(truncate_error_text("😀x", 2), "😀... [truncated 1 chars]");
    }
}
