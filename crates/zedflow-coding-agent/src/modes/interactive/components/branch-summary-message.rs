//! Collapsible branch-summary transcript entry.

use super::keybinding_hints::key_text;
use zedflow_agent::harness::messages::BranchSummaryMessage;
use zedflow_tui::{Box as TuiBox, Component, Markdown, Text};

pub struct BranchSummaryMessageComponent {
    message: BranchSummaryMessage,
    expanded: bool,
}
impl BranchSummaryMessageComponent {
    #[must_use]
    pub fn new(message: BranchSummaryMessage) -> Self {
        Self {
            message,
            expanded: false,
        }
    }
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}
impl Component for BranchSummaryMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut content = TuiBox::new(1, 1);
        content.add_child(Text::new("[branch]", 0, 0));
        if self.expanded {
            content.add_child(Markdown::new(format!(
                "**Branch Summary**\n\n{}",
                self.message.summary
            )));
        } else {
            content.add_child(Text::new(
                format!(
                    "Branch summary ({} to expand)",
                    key_text("app.tools.expand")
                ),
                0,
                0,
            ));
        }
        content.render(width)
    }
}
