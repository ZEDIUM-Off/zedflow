//! Diagnostic helpers ported from Pi's `packages/ai/src/utils/diagnostics.ts`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Object details attached to an assistant-message diagnostic.
pub type DiagnosticDetails = Map<String, Value>;

/// JavaScript `Error.code` values that Pi keeps in diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticErrorCode {
    /// String-valued error code.
    String(String),
    /// Number-valued error code.
    Number(f64),
}

/// Redacted error information stored in assistant-message diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticErrorInfo {
    /// Error constructor/name when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Error message, or the error name when Pi received an empty message.
    pub message: String,
    /// Error stack string when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
    /// Error code when it is a JavaScript string or number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DiagnosticErrorCode>,
}

/// Error-shaped thrown value used by the diagnostics port.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrownError {
    /// JavaScript `Error.name` value.
    pub name: String,
    /// JavaScript `Error.message` value.
    pub message: String,
    /// JavaScript `Error.stack` value when available.
    pub stack: Option<String>,
    /// JavaScript `Error.code` value when it is a string or number.
    pub code: Option<DiagnosticErrorCode>,
}

impl ThrownError {
    /// Creates an error-shaped thrown value.
    #[must_use]
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message: message.into(),
            stack: None,
            code: None,
        }
    }

    /// Sets the stack string on this error-shaped thrown value.
    #[must_use]
    pub fn with_stack(mut self, stack: impl Into<String>) -> Self {
        self.stack = Some(stack.into());
        self
    }

    /// Sets the string or number code on this error-shaped thrown value.
    #[must_use]
    pub fn with_code(mut self, code: DiagnosticErrorCode) -> Self {
        self.code = Some(code);
        self
    }
}

/// Thrown values accepted by the diagnostics port.
#[derive(Debug, Clone, PartialEq)]
pub enum ThrownValue {
    /// A JavaScript `Error` instance.
    Error(ThrownError),
    /// A JavaScript string value.
    String(String),
    /// Any other JavaScript value, already converted with JavaScript `String(value)` semantics.
    Other(String),
}

impl From<String> for ThrownValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ThrownValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<ThrownError> for ThrownValue {
    fn from(value: ThrownError) -> Self {
        Self::Error(value)
    }
}

/// Redacted provider/runtime diagnostic attached to an assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageDiagnostic {
    /// Diagnostic type string.
    #[serde(rename = "type")]
    pub type_: String,
    /// Unix timestamp in milliseconds, matching `Date.now()`.
    pub timestamp: i64,
    /// Extracted error information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DiagnosticErrorInfo>,
    /// Additional diagnostic details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<DiagnosticDetails>,
}

/// Formats a thrown value the way Pi's `formatThrownValue` does.
#[must_use]
pub fn format_thrown_value(value: &ThrownValue) -> String {
    match value {
        ThrownValue::Error(error) => {
            if error.message.is_empty() {
                error.name.clone()
            } else {
                error.message.clone()
            }
        }
        ThrownValue::String(value) | ThrownValue::Other(value) => value.clone(),
    }
}

/// Extracts redacted diagnostic error information from a thrown value.
#[must_use]
pub fn extract_diagnostic_error(error: &ThrownValue) -> DiagnosticErrorInfo {
    match error {
        ThrownValue::Error(error) => DiagnosticErrorInfo {
            name: if error.name.is_empty() {
                None
            } else {
                Some(error.name.clone())
            },
            message: if error.message.is_empty() {
                error.name.clone()
            } else {
                error.message.clone()
            },
            stack: error.stack.clone(),
            code: error.code.clone(),
        },
        value => DiagnosticErrorInfo {
            name: Some("ThrownValue".to_string()),
            message: format_thrown_value(value),
            stack: None,
            code: None,
        },
    }
}

/// Creates an assistant-message diagnostic for a thrown value.
#[must_use]
pub fn create_assistant_message_diagnostic(
    type_: impl Into<String>,
    error: &ThrownValue,
    details: Option<DiagnosticDetails>,
) -> AssistantMessageDiagnostic {
    AssistantMessageDiagnostic {
        type_: type_.into(),
        timestamp: date_now_millis(),
        error: Some(extract_diagnostic_error(error)),
        details,
    }
}

/// Appends a diagnostic to an optional diagnostics field.
pub fn append_assistant_message_diagnostic(
    diagnostics: &mut Option<Vec<AssistantMessageDiagnostic>>,
    diagnostic: AssistantMessageDiagnostic,
) {
    let mut next = diagnostics.take().unwrap_or_default();
    next.push(diagnostic);
    *diagnostics = Some(next);
}

fn date_now_millis() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(i64::MAX as u128) as i64,
        Err(error) => -(error.duration().as_millis().min(i64::MAX as u128) as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_extracts_thrown_values() {
        assert_eq!(format_thrown_value(&ThrownValue::from("plain")), "plain");
        assert_eq!(
            extract_diagnostic_error(&ThrownValue::Other("[object Object]".to_string())),
            DiagnosticErrorInfo {
                name: Some("ThrownValue".to_string()),
                message: "[object Object]".to_string(),
                stack: None,
                code: None,
            }
        );

        let error = ThrownError::new("TypeError", "")
            .with_stack("stack")
            .with_code(DiagnosticErrorCode::String("E_TYPE".to_string()));
        assert_eq!(
            format_thrown_value(&ThrownValue::Error(error.clone())),
            "TypeError"
        );
        assert_eq!(
            extract_diagnostic_error(&ThrownValue::Error(error)),
            DiagnosticErrorInfo {
                name: Some("TypeError".to_string()),
                message: "TypeError".to_string(),
                stack: Some("stack".to_string()),
                code: Some(DiagnosticErrorCode::String("E_TYPE".to_string())),
            }
        );
    }

    #[test]
    fn appends_diagnostics_to_optional_field() {
        let diagnostic = AssistantMessageDiagnostic {
            type_: "retry".to_string(),
            timestamp: 1,
            error: None,
            details: None,
        };
        let mut diagnostics = None;

        append_assistant_message_diagnostic(&mut diagnostics, diagnostic.clone());

        assert_eq!(diagnostics, Some(vec![diagnostic]));
    }
}
