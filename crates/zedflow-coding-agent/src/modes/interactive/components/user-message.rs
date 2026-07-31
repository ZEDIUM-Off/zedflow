//! Rendered user messages in the interactive transcript.

use zedflow_tui::{Component, Text};

const OSC133_ZONE_START: &str = "\x1b]133;A\x07";
const OSC133_ZONE_END: &str = "\x1b]133;B\x07\x1b]133;C\x07";

/// Pi's padded user-message transcript entry.
pub struct UserMessageComponent {
    text: String,
    output_pad: usize,
}

impl UserMessageComponent {
    #[must_use]
    pub fn new(text: impl Into<String>, output_pad: usize) -> Self {
        Self {
            text: text.into(),
            output_pad,
        }
    }

    pub fn set_output_pad(&mut self, output_pad: usize) {
        self.output_pad = output_pad;
    }
}

impl Component for UserMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Text::new(&self.text, self.output_pad, 1).render(width);
        if let Some(first) = lines.first_mut() {
            first.insert_str(0, OSC133_ZONE_START);
        }
        if let Some(last) = lines.last_mut() {
            last.insert_str(0, OSC133_ZONE_END);
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_messages_are_padded_and_marked_as_a_terminal_zone() {
        let lines = UserMessageComponent::new("hello", 1).render(8);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with(OSC133_ZONE_START));
        assert!(lines[2].starts_with(OSC133_ZONE_END));
    }
}
