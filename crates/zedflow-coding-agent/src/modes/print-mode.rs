//! Single-shot output mode helpers.

use serde::{Deserialize, Serialize};
use zedflow_ai::types::ImageContent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrintOutputMode {
    Text,
    Json,
}

impl Default for PrintOutputMode {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrintModeOptions {
    pub mode: PrintOutputMode,
    pub messages: Vec<String>,
    pub initial_message: Option<String>,
    pub initial_images: Vec<ImageContent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantResult {
    Text(String),
    Error(String),
    Aborted(String),
}

/// Render the final assistant result and return Pi's exit status.
pub fn render_print_result(result: &AssistantResult) -> (i32, String) {
    match result {
        AssistantResult::Text(text) => (0, format!("{text}\n")),
        AssistantResult::Error(error) => (1, error.clone()),
        AssistantResult::Aborted(reason) => (1, reason.clone()),
    }
}

/// Build prompts in the same order as Pi's print mode: initial prompt first,
/// followed by positional messages.
#[must_use]
pub fn prompts(options: &PrintModeOptions) -> Vec<String> {
    options
        .initial_message
        .iter()
        .cloned()
        .chain(options.messages.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_result_has_trailing_newline() {
        assert_eq!(
            render_print_result(&AssistantResult::Text("done".into())),
            (0, "done\n".into())
        );
    }
}
