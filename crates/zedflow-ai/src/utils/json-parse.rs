//! JSON repair and partial streaming parse helpers ported from Pi's `packages/ai/src/utils/json-parse.ts`.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

fn is_valid_json_escape(ch: char) -> bool {
    matches!(ch, '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u')
}

fn is_control_character(ch: char) -> bool {
    ch <= '\u{1f}'
}

fn escape_control_character(ch: char) -> String {
    match ch {
        '\u{0008}' => "\\b".to_string(),
        '\u{000c}' => "\\f".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        _ => format!("\\u{:04x}", u32::from(ch)),
    }
}

fn next_four_are_hex(chars: &[char], start: usize) -> bool {
    chars
        .get(start..start + 4)
        .is_some_and(|digits| digits.iter().all(|ch| ch.is_ascii_hexdigit()))
}

/// Repairs malformed JSON string literals by escaping raw control characters inside strings and
/// doubling backslashes before invalid escape characters.
#[must_use]
pub fn repair_json(json: &str) -> String {
    let chars = json.chars().collect::<Vec<_>>();
    let mut repaired = String::with_capacity(json.len());
    let mut in_string = false;
    let mut index = 0;

    while index < chars.len() {
        let ch = chars[index];

        if !in_string {
            repaired.push(ch);
            if ch == '"' {
                in_string = true;
            }
            index += 1;
            continue;
        }

        if ch == '"' {
            repaired.push(ch);
            in_string = false;
            index += 1;
            continue;
        }

        if ch == '\\' {
            let Some(&next_ch) = chars.get(index + 1) else {
                repaired.push_str("\\\\");
                index += 1;
                continue;
            };

            if next_ch == 'u' && next_four_are_hex(&chars, index + 2) {
                repaired.push_str("\\u");
                for digit in &chars[index + 2..index + 6] {
                    repaired.push(*digit);
                }
                index += 6;
                continue;
            }

            if is_valid_json_escape(next_ch) {
                repaired.push('\\');
                repaired.push(next_ch);
                index += 2;
                continue;
            }

            repaired.push_str("\\\\");
            index += 1;
            continue;
        }

        if is_control_character(ch) {
            repaired.push_str(&escape_control_character(ch));
        } else {
            repaired.push(ch);
        }
        index += 1;
    }

    repaired
}

/// Parses JSON and retries once after [`repair_json`] if the original parse fails.
///
/// # Errors
///
/// Returns the original serde JSON parse error when no repair is possible, or the repaired parse
/// error when repair changed the input but still does not produce valid JSON.
pub fn parse_json_with_repair<T>(json: &str) -> serde_json::Result<T>
where
    T: DeserializeOwned,
{
    match serde_json::from_str(json) {
        Ok(value) => Ok(value),
        Err(original_error) => {
            let repaired_json = repair_json(json);
            if repaired_json == json {
                Err(original_error)
            } else {
                serde_json::from_str(&repaired_json)
            }
        }
    }
}

/// Attempts to parse potentially incomplete JSON from a streaming response into a JSON value.
///
/// Returns an empty object when the input is missing, blank, or cannot be repaired into valid JSON.
#[must_use]
pub fn parse_streaming_json_value(partial_json: Option<&str>) -> Value {
    let Some(partial_json) = partial_json else {
        return empty_object();
    };
    if partial_json.trim().is_empty() {
        return empty_object();
    }

    if let Ok(value) = parse_json_with_repair(partial_json) {
        return value;
    }

    if let Some(value) = parse_partial_json(partial_json) {
        return value;
    }

    parse_partial_json(&repair_json(partial_json)).unwrap_or_else(empty_object)
}

/// Attempts to parse potentially incomplete JSON from a streaming response.
///
/// This mirrors Pi's `parseStreamingJson`: parse normally, try a partial parse, retry after JSON
/// repair, and fall back to an empty object. If `T` cannot be deserialized from the parsed value,
/// `T::default()` is returned.
#[must_use]
pub fn parse_streaming_json<T>(partial_json: Option<&str>) -> T
where
    T: DeserializeOwned + Default,
{
    serde_json::from_value(parse_streaming_json_value(partial_json)).unwrap_or_default()
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn parse_partial_json(partial_json: &str) -> Option<Value> {
    partial_json_candidates(partial_json)
        .into_iter()
        .find_map(|candidate| serde_json::from_str(&candidate).ok())
}

fn partial_json_candidates(partial_json: &str) -> Vec<String> {
    let completed = complete_partial_json(partial_json);
    let mut candidates = vec![completed.clone()];

    let mut trimmed = completed.trim_end().to_string();
    while let Some(ch) = trimmed.chars().last() {
        if matches!(ch, ',' | ':') || ch.is_ascii_alphabetic() || ch == '-' {
            trimmed.pop();
            candidates.push(complete_partial_json(&trimmed));
        } else {
            break;
        }
    }

    candidates
}

fn complete_partial_json(partial_json: &str) -> String {
    let mut output = partial_json.to_string();
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in partial_json.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' if stack.last() == Some(&ch) => {
                stack.pop();
            }
            _ => {}
        }
    }

    if in_string {
        if escape {
            output.push('\\');
        }
        output.push('"');
    }

    complete_trailing_value(&mut output);
    trim_trailing_commas(&mut output);

    while let Some(ch) = stack.pop() {
        output.push(ch);
    }

    output
}

fn complete_trailing_value(output: &mut String) {
    let trimmed = output.trim_end();
    if trimmed.ends_with(':') {
        output.push_str("null");
        return;
    }

    for (partial, suffix) in [
        ("t", "rue"),
        ("tr", "ue"),
        ("tru", "e"),
        ("f", "alse"),
        ("fa", "lse"),
        ("fal", "se"),
        ("fals", "e"),
        ("n", "ull"),
        ("nu", "ll"),
        ("nul", "l"),
    ] {
        if trimmed.ends_with(partial) {
            output.push_str(suffix);
            return;
        }
    }
}

fn trim_trailing_commas(output: &mut String) {
    loop {
        let Some(ch) = output.chars().last() else {
            return;
        };
        if ch.is_whitespace() {
            output.pop();
            continue;
        }
        if ch == ',' {
            output.pop();
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn repairs_control_characters_and_invalid_escapes_inside_strings() {
        assert_eq!(
            repair_json("{\"a\":\"line\nbad\\q\"}"),
            "{\"a\":\"line\\nbad\\\\q\"}"
        );
        assert_eq!(
            parse_json_with_repair::<Value>("{\"a\":\"line\nbad\\q\"}").unwrap(),
            json!({ "a": "line\nbad\\q" })
        );
    }

    #[test]
    fn keeps_valid_unicode_escapes() {
        assert_eq!(repair_json("{\"a\":\"\\u263a\"}"), "{\"a\":\"\\u263a\"}");
        assert_eq!(
            parse_json_with_repair::<Value>("{\"a\":\"\\u263a\"}").unwrap(),
            json!({ "a": "☺" })
        );
    }

    #[test]
    fn parses_incomplete_streaming_objects() {
        assert_eq!(
            parse_streaming_json_value(Some("{\"a\":{\"b\":\"hel")),
            json!({ "a": { "b": "hel" } })
        );
        assert_eq!(
            parse_streaming_json_value(Some("{\"a\":true,")),
            json!({ "a": true })
        );
        assert_eq!(parse_streaming_json_value(None), json!({}));
    }

    #[test]
    fn parses_repaired_incomplete_streaming_tool_args() {
        assert_eq!(
            parse_streaming_json_value(Some("{\"path\":\"C:\\z\",\"lines\":[1,")),
            json!({ "path": "C:\\z", "lines": [1] })
        );
        assert_eq!(
            parse_streaming_json_value(Some("{\"text\":\"line\nbad\\q")),
            json!({ "text": "line\nbad\\q" })
        );
    }
}
