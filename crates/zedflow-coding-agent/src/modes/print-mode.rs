//! Single-shot (text and JSON event stream) mode helpers.

use serde::Serialize;
use serde_json::Value;

use super::rpc::jsonl::serialize_json_line;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOutputMode {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintModeOptions {
    pub mode: PrintOutputMode,
    pub initial_message: Option<String>,
    pub messages: Vec<String>,
}

impl Default for PrintModeOptions {
    fn default() -> Self {
        Self {
            mode: PrintOutputMode::Text,
            initial_message: None,
            messages: Vec::new(),
        }
    }
}

/// Extracts the text from the final assistant payload used by text mode.
pub fn render_text_response(payload: &Value) -> Option<String> {
    let content = payload.get("content")?.as_array()?;
    let text: String = content
        .iter()
        .filter_map(|block| {
            (block.get("type")?.as_str() == Some("text"))
                .then(|| block.get("text")?.as_str())
                .flatten()
        })
        .collect();
    (!text.is_empty()).then_some(text)
}

pub fn serialize_event<T: Serialize>(event: &T) -> serde_json::Result<String> {
    serialize_json_line(event)
}

/// Small runtime-neutral print-mode seam. The caller supplies one response per prompt.
pub fn run_print_mode<F>(options: &PrintModeOptions, mut prompt: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<Value, String>,
{
    let mut prompts = Vec::new();
    if let Some(message) = &options.initial_message {
        prompts.push(message.as_str());
    }
    prompts.extend(options.messages.iter().map(String::as_str));
    let mut output = String::new();
    for message in prompts {
        let payload = prompt(message)?;
        match options.mode {
            PrintOutputMode::Text => {
                if let Some(text) = render_text_response(&payload) {
                    output.push_str(&text);
                    output.push('\n');
                }
            }
            PrintOutputMode::Json => {
                output.push_str(&serialize_json_line(&payload).map_err(|e| e.to_string())?)
            }
        }
    }
    Ok(output)
}
