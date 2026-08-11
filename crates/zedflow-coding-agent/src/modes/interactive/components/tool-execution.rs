//! Generic incremental rendering for tool calls and results.

use zedflow_tui::{Component, Text};

use super::visual_truncate::truncate_to_visual_lines;

const PREVIEW_LINES: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResultContent {
    Text(String),
    Image { mime_type: String },
}

pub struct ToolExecutionComponent {
    tool_name: String,
    arguments: String,
    result: Option<(Vec<ToolResultContent>, bool)>,
    expanded: bool,
    execution_started: bool,
    args_complete: bool,
    is_partial: bool,
    show_images: bool,
}

impl ToolExecutionComponent {
    #[must_use]
    pub fn new(tool_name: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            arguments: arguments.into(),
            result: None,
            expanded: false,
            execution_started: false,
            args_complete: false,
            is_partial: true,
            show_images: true,
        }
    }

    pub fn update_args(&mut self, arguments: impl Into<String>) {
        self.arguments = arguments.into();
    }

    pub fn mark_execution_started(&mut self) {
        self.execution_started = true;
    }

    pub fn set_args_complete(&mut self) {
        self.args_complete = true;
    }

    pub fn update_result(&mut self, result: impl Into<String>, is_error: bool) {
        self.update_result_content(
            vec![ToolResultContent::Text(result.into())],
            is_error,
            false,
        );
    }

    pub fn update_result_content(
        &mut self,
        content: Vec<ToolResultContent>,
        is_error: bool,
        is_partial: bool,
    ) {
        self.result = Some((content, is_error));
        self.is_partial = is_partial;
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn set_show_images(&mut self, show: bool) {
        self.show_images = show;
    }

    #[must_use]
    pub fn is_partial(&self) -> bool {
        self.is_partial
    }
}

impl Component for ToolExecutionComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut text = self.tool_name.clone();
        if !self.arguments.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.arguments);
        }
        if let Some((content, is_error)) = &self.result {
            let output = content
                .iter()
                .filter_map(|block| match block {
                    ToolResultContent::Text(text) => Some(text.as_str()),
                    ToolResultContent::Image { .. } if !self.show_images => Some("[Image]"),
                    ToolResultContent::Image { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !output.is_empty() {
                text.push('\n');
                if *is_error {
                    text.push_str("Error: ");
                }
                if self.expanded {
                    text.push_str(&output);
                } else {
                    let truncated = truncate_to_visual_lines(&output, PREVIEW_LINES, width, 0);
                    text.push_str(&truncated.visual_lines.join("\n"));
                    if truncated.skipped_count > 0 {
                        text.push_str(&format!("\n… {} lines hidden", truncated.skipped_count));
                    }
                }
            }
        } else if self.execution_started {
            // Pi's generic tool fallback shows the call itself until a result arrives.
        }
        Text::new(text, 1, 1).render(width)
    }
}
