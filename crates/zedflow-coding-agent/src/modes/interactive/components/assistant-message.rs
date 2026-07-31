//! Pi-compatible assistant transcript rendering.

use zedflow_tui::{Component, Text};

const OSC133_ZONE_START: &str = "\x1b]133;A\x07";
const OSC133_ZONE_END: &str = "\x1b]133;B\x07\x1b]133;C\x07";

/// The visible content accumulated while an assistant message streams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamingAssistantMessage {
    thinking: String,
    text: String,
    hide_thinking: bool,
    hidden_thinking_label: String,
    output_pad: usize,
}

impl StreamingAssistantMessage {
    /// Replaces the streamed snapshot, rather than appending it twice.
    pub fn update_content(&mut self, thinking: impl Into<String>, text: impl Into<String>) {
        self.thinking = thinking.into();
        self.text = text.into();
    }

    pub fn set_hide_thinking(&mut self, hide_thinking: bool) {
        self.hide_thinking = hide_thinking;
    }

    pub fn set_output_pad(&mut self, output_pad: usize) {
        self.output_pad = output_pad;
    }

    #[must_use]
    pub fn thinking(&self) -> &str {
        &self.thinking
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Component for StreamingAssistantMessage {
    fn render(&self, width: usize) -> Vec<String> {
        let mut content = Vec::new();
        if !self.thinking.trim().is_empty() {
            content.push(if self.hide_thinking {
                if self.hidden_thinking_label.is_empty() {
                    "Thinking..."
                } else {
                    &self.hidden_thinking_label
                }
                .to_owned()
            } else {
                self.thinking.trim().to_owned()
            });
        }
        if !self.text.trim().is_empty() {
            content.push(self.text.trim().to_owned());
        }
        let mut lines = Text::new(
            content.join("\n\n"),
            self.output_pad,
            usize::from(!content.is_empty()),
        )
        .render(width);
        if !lines.is_empty() {
            lines[0].insert_str(0, OSC133_ZONE_START);
            let last = lines.len() - 1;
            lines[last].insert_str(0, OSC133_ZONE_END);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_thinking_uses_a_label_without_dropping_the_response() {
        let mut message = StreamingAssistantMessage::default();
        message.update_content("private", "answer");
        message.set_hide_thinking(true);
        let rendered = message.render(80).join("\n");
        assert!(rendered.contains("Thinking..."));
        assert!(rendered.contains("answer"));
        assert!(!rendered.contains("private"));
    }
}
