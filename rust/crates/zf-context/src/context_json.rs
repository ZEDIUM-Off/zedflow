//! Bounded JSON decoding for the structured context grammar and its portable exports.
//! A 64-level record uses more than 128 JSON containers: the transport envelope
//! and each `fields` map must not reduce the language's semantic depth limit.
use serde::de::DeserializeOwned;
use std::{error::Error, fmt};

const MAX_JSON_DEPTH: usize = 256;
const LOCAL_STACK_DEPTH: usize = 48;
const DECODE_STACK_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct DecodeError(String);
impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for DecodeError {}

/// JSON punctuation inside strings is data. Escapes are consumed before looking
/// for the next quotation mark; complete syntax checking remains Serde's job.
fn structural_depth(bytes: &[u8]) -> Result<usize, DecodeError> {
    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    maximum = maximum.max(depth);
                    if depth > MAX_JSON_DEPTH {
                        return Err(DecodeError(format!(
                            "JSON structure exceeds {MAX_JSON_DEPTH} container levels"
                        )));
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(maximum)
}

/// Semantic validators still enforce their 64-level limit. This codec only
/// permits the JSON representation of that grammar, with a separate hard guard.
pub fn from_slice<T: DeserializeOwned + Send>(bytes: &[u8]) -> Result<T, DecodeError> {
    let depth = structural_depth(bytes)?;
    let decode = || {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        deserializer.disable_recursion_limit();
        let value = T::deserialize(&mut deserializer).map_err(|e| DecodeError(e.to_string()))?;
        deserializer.end().map_err(|e| DecodeError(e.to_string()))?;
        Ok(value)
    };
    if depth <= LOCAL_STACK_DEPTH {
        return decode();
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("zedflow-context-json".into())
            .stack_size(DECODE_STACK_BYTES)
            .spawn_scoped(scope, decode)
            .map_err(|e| DecodeError(format!("Cannot start context decoder: {e}")))?
            .join()
            .map_err(|_| DecodeError("Context decoder failed".into()))?
    })
}

pub fn from_str<T: DeserializeOwned + Send>(text: &str) -> Result<T, DecodeError> {
    from_slice(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn json_guard_ignores_escaped_strings_but_rejects_excessive_structure() {
        let value = json!({"literal":format!("\\\"{}{}", "[".repeat(1000), "}".repeat(1000))});
        assert_eq!(from_str::<Value>(&value.to_string()).unwrap(), value);
        let excessive = format!("{}0{}", "[".repeat(257), "]".repeat(257));
        assert!(
            from_str::<Value>(&excessive)
                .unwrap_err()
                .to_string()
                .contains("256")
        );
        assert!(from_str::<Value>("{} {}").is_err());
        assert!(from_str::<Value>("{\"unterminated\":").is_err());
    }
}
