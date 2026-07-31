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

/// Pi enters print mode for an explicit print flag or either redirected stream.
#[must_use]
pub fn should_run_print(explicit_print: bool, stdin_is_tty: bool, stdout_is_tty: bool) -> bool {
    explicit_print || !stdin_is_tty || !stdout_is_tty
}

/// Piped whitespace is not a prompt.
#[must_use]
pub fn piped_initial_message(input: String) -> Option<String> {
    let input = input.trim().to_owned();
    (!input.is_empty()).then_some(input)
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

    #[test]
    fn redirected_streams_select_print_and_trim_piped_input() {
        assert!(should_run_print(false, false, true));
        assert!(should_run_print(false, true, false));
        assert!(!should_run_print(false, true, true));
        assert_eq!(
            piped_initial_message("  piped prompt\n".into()).as_deref(),
            Some("piped prompt")
        );
        assert_eq!(piped_initial_message(" \n".into()), None);
    }
}
