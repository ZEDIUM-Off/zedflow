//! Collapsible compaction-summary transcript entry.

use super::keybinding_hints::key_text;
use zedflow_agent::harness::messages::CompactionSummaryMessage;
use zedflow_tui::{Box as TuiBox, Component, Markdown, Text};

pub struct CompactionSummaryMessageComponent {
    message: CompactionSummaryMessage,
    expanded: bool,
}
impl CompactionSummaryMessageComponent {
    #[must_use]
    pub fn new(message: CompactionSummaryMessage) -> Self {
        Self {
            message,
            expanded: false,
        }
    }
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}
impl Component for CompactionSummaryMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut content = TuiBox::new(1, 1);
        content.add_child(Text::new("[compaction]", 0, 0));
        if self.expanded {
            content.add_child(Markdown::new(format!(
                "**Compacted from {} tokens**\n\n{}",
                self.message.tokens_before, self.message.summary
            )));
        } else {
            content.add_child(Text::new(
                format!(
                    "Compacted from {} tokens ({} to expand)",
                    self.message.tokens_before,
                    key_text("app.tools.expand")
                ),
                0,
                0,
            ));
        }
        content.render(width)
    }
}
