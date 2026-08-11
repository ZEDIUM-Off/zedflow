//! Collapsible skill invocation transcript entry.

use super::keybinding_hints::key_text;
use zedflow_tui::{Box as TuiBox, Component, Markdown, Text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkillBlock {
    pub name: String,
    pub content: String,
}

pub struct SkillInvocationMessageComponent {
    skill: ParsedSkillBlock,
    expanded: bool,
}
impl SkillInvocationMessageComponent {
    #[must_use]
    pub fn new(skill: ParsedSkillBlock) -> Self {
        Self {
            skill,
            expanded: false,
        }
    }
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }
}
impl Component for SkillInvocationMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut content = TuiBox::new(1, 1);
        if self.expanded {
            content.add_child(Text::new("[skill]", 0, 0));
            content.add_child(Markdown::new(format!(
                "**{}**\n\n{}",
                self.skill.name, self.skill.content
            )));
        } else {
            content.add_child(Text::new(
                format!(
                    "[skill] {} ({} to expand)",
                    self.skill.name,
                    key_text("app.tools.expand")
                ),
                0,
                0,
            ));
        }
        content.render(width)
    }
}
