//! Generic interactive rendering for tool calls and results.

use zedflow_tui::{Component, Text};

use super::visual_truncate::truncate_to_visual_lines;

/// Pi's fallback tool renderer when a tool has no custom renderer.
pub struct ToolExecutionComponent {
    tool_name: String,
    arguments: String,
    result: Option<(String, bool)>,
    expanded: bool,
}

impl ToolExecutionComponent {
    #[must_use]
    pub fn new(tool_name: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            arguments: arguments.into(),
            result: None,
            expanded: false,
        }
    }

    pub fn update_result(&mut self, result: impl Into<String>, is_error: bool) {
        self.result = Some((result.into(), is_error));
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}

impl Component for ToolExecutionComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut text = self.tool_name.clone();
        if !self.arguments.is_empty() {
            text.push_str("\n\n");
            text.push_str(&self.arguments);
        }
        if let Some((result, is_error)) = &self.result {
            text.push('\n');
            if *is_error {
                text.push_str("Error: ");
            }
            if self.expanded {
                text.push_str(result);
            } else {
                let truncated = truncate_to_visual_lines(result, 10, width, 1);
                text.push_str(&truncated.visual_lines.join("\n"));
                if truncated.skipped_count > 0 {
                    text.push_str(&format!("\n… {} lines hidden", truncated.skipped_count));
                }
            }
        }
        Text::new(text, 1, 1).render(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_tool_output_reports_hidden_lines() {
        let mut tool = ToolExecutionComponent::new("read", "{}");
        tool.update_result(
            (0..12)
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            false,
        );
        assert!(
            tool.render(80)
                .iter()
                .any(|line| line.contains("2 lines hidden"))
        );
    }
}
