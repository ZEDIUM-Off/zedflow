//! Strict LF-delimited JSON framing.

use serde::Serialize;
use serde_json::Value;

#[must_use]
pub fn serialize_json_line<T: Serialize>(value: &T) -> String {
    let mut line = serde_json::to_string(value).expect("JSON serialization should not fail");
    line.push('\n');
    line
}

#[derive(Debug, Default)]
pub struct JsonlReader {
    buffer: Vec<u8>,
}

impl JsonlReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed bytes and return complete records. Only LF frames records; U+2028
    /// and U+2029 remain ordinary JSON string contents.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let bytes: Vec<_> = self.buffer.drain(..=index).collect();
            let line = &bytes[..bytes.len() - 1];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            lines.push(String::from_utf8_lossy(line).into_owned());
        }
        lines
    }

    pub fn finish(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }
        let line = std::mem::take(&mut self.buffer);
        let line = line.strip_suffix(b"\r").unwrap_or(&line);
        Some(String::from_utf8_lossy(line).into_owned())
    }

    pub fn parse<T: serde::de::DeserializeOwned>(line: &str) -> serde_json::Result<T> {
        serde_json::from_str::<Value>(line).and_then(serde_json::from_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_only_on_lf() {
        let line = serialize_json_line(&serde_json::json!({"text":"a\u{2028}b\u{2029}c"}));
        let mut reader = JsonlReader::new();
        assert_eq!(reader.push(line.as_bytes()).len(), 1);
        assert_eq!(reader.finish(), None);
    }
    #[test]
    fn supports_crlf_and_final_record() {
        let mut reader = JsonlReader::new();
        assert_eq!(reader.push(b"{\"a\":1}\r\n{\"b\":2}"), vec![r#"{"a":1}"#]);
        assert_eq!(reader.finish().as_deref(), Some(r#"{"b":2}"#));
    }
}
