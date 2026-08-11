//! Extension-defined custom message transcript entries.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use serde_json::Value;
use zedflow_agent::harness::messages::{CustomMessage, CustomMessageContent};
use zedflow_ai::UserContentBlock;
use zedflow_tui::{Box as TuiBox, Component, Markdown, Text};

pub type MessageRenderer =
    Arc<dyn Fn(&CustomMessage<Value>, bool) -> Option<Box<dyn Component>> + Send + Sync>;

pub struct CustomMessageComponent {
    message: CustomMessage<Value>,
    renderer: Option<MessageRenderer>,
    custom_component: Option<Box<dyn Component>>,
    expanded: bool,
}

impl CustomMessageComponent {
    #[must_use]
    pub fn new(message: CustomMessage<Value>, renderer: Option<MessageRenderer>) -> Self {
        let mut result = Self {
            message,
            renderer,
            custom_component: None,
            expanded: false,
        };
        result.rebuild();
        result
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded != expanded {
            self.expanded = expanded;
            self.rebuild();
        }
    }

    fn rebuild(&mut self) {
        self.custom_component = self.renderer.as_ref().and_then(|renderer| {
            catch_unwind(AssertUnwindSafe(|| renderer(&self.message, self.expanded)))
                .ok()
                .flatten()
        });
    }

    fn default_component(&self) -> TuiBox {
        let mut content = TuiBox::new(1, 1);
        content.add_child(Text::new(format!("[{}]", self.message.custom_type), 0, 0));
        let text = match &self.message.content {
            CustomMessageContent::Text(text) => text.clone(),
            CustomMessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    UserContentBlock::Text(text) => Some(text.text.as_str()),
                    UserContentBlock::Image(_) => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        content.add_child(Markdown::new(text));
        content
    }
}

impl Component for CustomMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![String::new()];
        if let Some(component) = &self.custom_component {
            lines.extend(component.render(width));
        } else {
            lines.extend(self.default_component().render(width));
        }
        lines
    }

    fn invalidate(&mut self) {
        self.rebuild();
    }
}
