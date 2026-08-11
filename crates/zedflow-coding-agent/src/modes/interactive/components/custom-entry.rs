//! Extension-defined custom session entries.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use serde_json::Value;
use zedflow_agent::harness::types::CustomEntry;
use zedflow_tui::{Box as TuiBox, Component, Text};

pub type EntryRenderer =
    Arc<dyn Fn(&CustomEntry<Value>, bool) -> Option<Box<dyn Component>> + Send + Sync>;

pub struct CustomEntryComponent {
    entry: CustomEntry<Value>,
    renderer: EntryRenderer,
    component: Option<Box<dyn Component>>,
    expanded: bool,
}

impl CustomEntryComponent {
    #[must_use]
    pub fn new(entry: CustomEntry<Value>, renderer: EntryRenderer) -> Self {
        let mut result = Self {
            entry,
            renderer,
            component: None,
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

    #[must_use]
    pub fn has_content(&self) -> bool {
        self.component.is_some()
    }

    fn rebuild(&mut self) {
        self.component = match catch_unwind(AssertUnwindSafe(|| {
            (self.renderer)(&self.entry, self.expanded)
        })) {
            Ok(component) => component,
            Err(error) => {
                let message = error
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| error.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("unknown error");
                let mut box_component = TuiBox::new(1, 1);
                box_component.add_child(Text::new(
                    format!("[{}] renderer failed: {message}", self.entry.custom_type),
                    0,
                    0,
                ));
                Some(Box::new(box_component))
            }
        };
    }
}

impl Component for CustomEntryComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let Some(component) = &self.component else {
            return Vec::new();
        };
        let mut lines = vec![String::new()];
        lines.extend(component.render(width));
        lines
    }

    fn invalidate(&mut self) {
        self.rebuild();
    }
}
