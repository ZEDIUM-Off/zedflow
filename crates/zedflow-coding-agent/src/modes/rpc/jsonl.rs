//! Strict LF-delimited JSON framing.

use serde::Serialize;
use serde_json::Value;

pub fn serialize_json_line<T: Serialize>(value: &T) -> serde_json::Result<String> {
    serde_json::to_string(value).map(|json| format!("{json}\n"))
}

/// Splits only on LF. CRLF is accepted by removing the framing CR.
pub fn parse_json_lines(input: &str) -> impl Iterator<Item = &str> {
    input
        .split('\n')
        .filter_map(|line| line.strip_suffix('\r').or(Some(line)))
        .filter(|line| !line.is_empty())
}

pub fn decode_json_lines(input: &str) -> Result<Vec<Value>, serde_json::Error> {
    parse_json_lines(input).map(serde_json::from_str).collect()
}
