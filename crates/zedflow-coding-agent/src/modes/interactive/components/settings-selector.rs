//! Pi's settings selector, rendered by the shared TUI `SettingsList`.

use std::sync::{Arc, Mutex};

use zedflow_tui::{Component, SettingItem, SettingsList, SettingsListTheme};

use crate::modes_interactive_theme_theme::Theme;

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
        Self::with_theme(items, SettingsListTheme::default())
    }

    #[must_use]
    pub fn with_theme(items: Vec<SettingChoice>, theme: SettingsListTheme) -> Self {
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
            theme,
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

    #[must_use]
    pub fn theme(theme: Arc<Mutex<Theme>>) -> SettingsListTheme {
        let paint = |token: &'static str, text: &str| {
            let theme = theme.lock().unwrap();
            theme.fg(token, text).unwrap_or_else(|_| text.to_owned())
        };
        let label_theme = Arc::clone(&theme);
        let value_theme = Arc::clone(&theme);
        let description_theme = Arc::clone(&theme);
        let hint_theme = Arc::clone(&theme);
        let cursor = paint("accent", "→ ");
        SettingsListTheme {
            label: Arc::new(move |text, selected| {
                if selected {
                    label_theme
                        .lock()
                        .unwrap()
                        .fg("accent", text)
                        .unwrap_or_else(|_| text.to_owned())
                } else {
                    text.to_owned()
                }
            }),
            value: Arc::new(move |text, selected| {
                let token = if selected { "accent" } else { "muted" };
                value_theme
                    .lock()
                    .unwrap()
                    .fg(token, text)
                    .unwrap_or_else(|_| text.to_owned())
            }),
            description: Arc::new(move |text| {
                description_theme
                    .lock()
                    .unwrap()
                    .fg("dim", text)
                    .unwrap_or_else(|_| text.to_owned())
            }),
            cursor,
            hint: Arc::new(move |text| {
                hint_theme
                    .lock()
                    .unwrap()
                    .fg("dim", text)
                    .unwrap_or_else(|_| text.to_owned())
            }),
        }
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
