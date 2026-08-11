//! Incremental assistant transcript rendering.

use zedflow_tui::{Component, Markdown, Text};

const OSC133_ZONE_START: &str = "\x1b]133;A\x07";
const OSC133_ZONE_END: &str = "\x1b]133;B\x07\x1b]133;C\x07";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantContent {
    Text(String),
    Thinking(String),
    ToolCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Complete,
    Length,
    Aborted,
    Error,
}

/// The latest assistant snapshot. Updates replace previous streamed snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamingAssistantMessage {
    content: Vec<AssistantContent>,
    hide_thinking: bool,
    hidden_thinking_label: String,
    output_pad: usize,
    stop_reason: StopReason,
    error_message: Option<String>,
}

impl Default for StreamingAssistantMessage {
    fn default() -> Self {
        Self {
            content: Vec::new(),
            hide_thinking: false,
            hidden_thinking_label: "Thinking...".into(),
            output_pad: 1,
            stop_reason: StopReason::Complete,
            error_message: None,
        }
    }
}

impl StreamingAssistantMessage {
    pub fn update_content(&mut self, thinking: impl Into<String>, text: impl Into<String>) {
        self.content = vec![
            AssistantContent::Thinking(thinking.into()),
            AssistantContent::Text(text.into()),
        ];
    }

    pub fn update_snapshot(&mut self, content: Vec<AssistantContent>) {
        self.content = content;
    }

    pub fn set_hide_thinking(&mut self, hide: bool) {
        self.hide_thinking = hide;
    }

    pub fn set_hidden_thinking_label(&mut self, label: impl Into<String>) {
        self.hidden_thinking_label = label.into();
    }

    pub fn set_output_pad(&mut self, padding: usize) {
        self.output_pad = padding;
    }

    pub fn set_stop(&mut self, reason: StopReason, error_message: Option<String>) {
        self.stop_reason = reason;
        self.error_message = error_message;
    }

    #[must_use]
    pub fn thinking(&self) -> &str {
        self.content
            .iter()
            .find_map(|part| match part {
                AssistantContent::Thinking(value) => Some(value.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }

    #[must_use]
    pub fn text(&self) -> &str {
        self.content
            .iter()
            .find_map(|part| match part {
                AssistantContent::Text(value) => Some(value.as_str()),
                _ => None,
            })
            .unwrap_or("")
    }
}

impl Component for StreamingAssistantMessage {
    fn render(&self, width: usize) -> Vec<String> {
        let has_tool_calls = self
            .content
            .iter()
            .any(|part| matches!(part, AssistantContent::ToolCall));
        let visible: Vec<_> = self
            .content
            .iter()
            .filter(|part| match part {
                AssistantContent::Text(text) | AssistantContent::Thinking(text) => {
                    !text.trim().is_empty()
                }
                AssistantContent::ToolCall => false,
            })
            .collect();
        let mut lines = Vec::new();
        if !visible.is_empty() {
            lines.push(String::new());
        }
        for (index, part) in visible.iter().enumerate() {
            let text = match part {
                AssistantContent::Text(text) => text.trim(),
                AssistantContent::Thinking(text) if !self.hide_thinking => text.trim(),
                AssistantContent::Thinking(_) => &self.hidden_thinking_label,
                AssistantContent::ToolCall => unreachable!(),
            };
            lines.extend(
                Markdown::new(text)
                    .with_padding(self.output_pad, 0)
                    .render(width),
            );
            if index + 1 < visible.len() && matches!(part, AssistantContent::Thinking(_)) {
                lines.push(String::new());
            }
        }
        let error = match self.stop_reason {
            StopReason::Length => Some("Error: Model stopped because it reached the maximum output token limit. The response may be incomplete.".to_owned()),
            StopReason::Aborted if !has_tool_calls => Some(self.error_message.as_deref().filter(|message| *message != "Request was aborted").unwrap_or("Operation aborted").to_owned()),
            StopReason::Error if !has_tool_calls => Some(format!("Error: {}", self.error_message.as_deref().unwrap_or("Unknown error"))),
            _ => None,
        };
        if let Some(error) = error {
            lines.push(String::new());
            lines.extend(Text::new(error, self.output_pad, 0).render(width));
        }
        if !has_tool_calls && !lines.is_empty() {
            lines[0].insert_str(0, OSC133_ZONE_START);
            let last = lines.len() - 1;
            lines[last].insert_str(0, OSC133_ZONE_END);
        }
        lines
    }
}
