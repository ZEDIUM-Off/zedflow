//! Pi's settings selector, rendered by the shared TUI `SettingsList`.

use std::sync::{Arc, Mutex};

use zedflow_tui::{Component, SettingItem, SettingsList, SettingsListTheme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingChoice {
    pub id: String,
    pub label: String,
    pub description: String,
    pub value: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsAction {
    Change { id: String, value: String },
    Cancel,
}

pub struct SettingsSelector {
    list: SettingsList,
    actions: Arc<Mutex<Vec<SettingsAction>>>,
}

impl SettingsSelector {
    #[must_use]
    pub fn new(items: Vec<SettingChoice>) -> Self {
        let actions = Arc::new(Mutex::new(Vec::new()));
        let mut list = SettingsList::with_options(
            items
                .into_iter()
                .map(|item| SettingItem {
                    id: item.id,
                    label: item.label,
                    description: Some(item.description),
                    current_value: item.value,
                    values: item.values,
                })
                .collect(),
            10,
            SettingsListTheme::default(),
            true,
        );
        let changes = Arc::clone(&actions);
        list.on_change = Some(Box::new(move |id, value| {
            changes.lock().unwrap().push(SettingsAction::Change {
                id: id.into(),
                value: value.into(),
            });
        }));
        let cancels = Arc::clone(&actions);
        list.on_cancel = Some(Box::new(move || {
            cancels.lock().unwrap().push(SettingsAction::Cancel);
        }));
        Self { list, actions }
    }

    pub fn filter(&mut self, query: &str) {
        self.list.set_filter(query);
    }

    pub fn move_selection(&mut self, delta: isize) {
        self.list
            .handle_input(if delta < 0 { "\x1b[A" } else { "\x1b[B" });
    }

    pub fn activate(&mut self) -> Option<SettingsAction> {
        self.list.handle_input("\r");
        self.take_action()
    }

    #[must_use]
    pub const fn cancel(&self) -> SettingsAction {
        SettingsAction::Cancel
    }

    pub fn handle_input(&mut self, data: &str) {
        self.list.handle_input(data);
    }

    fn take_action(&self) -> Option<SettingsAction> {
        self.actions.lock().unwrap().pop()
    }

    pub fn drain_actions(&self) -> Vec<SettingsAction> {
        std::mem::take(&mut *self.actions.lock().unwrap())
    }
}

impl Component for SettingsSelector {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec!["─".repeat(width.max(1))];
        lines.extend(self.list.render(width));
        lines.push("─".repeat(width.max(1)));
        lines
    }

    fn handle_input(&mut self, data: &str) {
        self.handle_input(data);
    }
}
