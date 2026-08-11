use crate::{
    Component,
    fuzzy::fuzzy_filter,
    get_keybindings,
    utils::{truncate_to_width, visible_width, wrap_text_with_ansi},
};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub current_value: String,
    pub values: Vec<String>,
}
pub type SettingsListStyle = Arc<dyn Fn(&str, bool) -> String + Send + Sync>;
pub type SettingsListTextStyle = Arc<dyn Fn(&str) -> String + Send + Sync>;
#[derive(Clone)]
pub struct SettingsListTheme {
    pub label: SettingsListStyle,
    pub value: SettingsListStyle,
    pub description: SettingsListTextStyle,
    pub cursor: String,
    pub hint: SettingsListTextStyle,
}
impl Default for SettingsListTheme {
    fn default() -> Self {
        Self {
            label: Arc::new(|s, _| s.into()),
            value: Arc::new(|s, _| s.into()),
            description: Arc::new(str::to_owned),
            cursor: "→ ".into(),
            hint: Arc::new(str::to_owned),
        }
    }
}

pub struct SettingsList {
    pub items: Vec<SettingItem>,
    filtered_items: Vec<SettingItem>,
    pub selected: usize,
    pub max_visible: usize,
    pub on_change: Option<Box<dyn FnMut(&str, &str)>>,
    pub on_cancel: Option<Box<dyn FnMut()>>,
    theme: SettingsListTheme,
    search_enabled: bool,
    search: String,
}
impl SettingsList {
    pub fn new(items: Vec<SettingItem>, max_visible: usize) -> Self {
        Self::with_options(items, max_visible, SettingsListTheme::default(), false)
    }
    pub fn with_options(
        items: Vec<SettingItem>,
        max_visible: usize,
        theme: SettingsListTheme,
        search_enabled: bool,
    ) -> Self {
        Self {
            filtered_items: items.clone(),
            items,
            selected: 0,
            max_visible,
            on_change: None,
            on_cancel: None,
            theme,
            search_enabled,
            search: String::new(),
        }
    }
    pub fn update_value(&mut self, id: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(i) = self.items.iter_mut().find(|i| i.id == id) {
            i.current_value = value;
        }
        self.apply_filter();
    }
    pub fn set_filter(&mut self, query: &str) {
        self.search = query.into();
        self.apply_filter();
    }
    fn apply_filter(&mut self) {
        self.filtered_items = fuzzy_filter(&self.items, &self.search, |i| &i.label);
        self.selected = 0;
    }
    fn hint(&self) -> &'static str {
        if self.search_enabled {
            "  Type to search · Enter/Space to change · Esc to cancel"
        } else {
            "  Enter/Space to change · Esc to cancel"
        }
    }
    fn activate(&mut self) {
        let Some(item) = self.filtered_items.get_mut(self.selected) else {
            return;
        };
        if item.values.is_empty() {
            return;
        }
        let current = item
            .values
            .iter()
            .position(|v| v == &item.current_value)
            .unwrap_or(0);
        let value = item.values[(current + 1) % item.values.len()].clone();
        item.current_value = value.clone();
        if let Some(source) = self.items.iter_mut().find(|i| i.id == item.id) {
            source.current_value = value.clone()
        }
        if let Some(callback) = &mut self.on_change {
            callback(&item.id, &value)
        }
    }
}
impl Component for SettingsList {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        if self.search_enabled {
            lines.push(truncate_to_width(
                &format!("> {}", self.search),
                width,
                "",
                false,
            ));
            lines.push(String::new());
        }
        if self.items.is_empty() {
            lines.push((self.theme.hint)("  No settings available"));
            lines.push(String::new());
            lines.push((self.theme.hint)(self.hint()));
            return lines;
        }
        if self.filtered_items.is_empty() {
            lines.push((self.theme.hint)("  No matching settings"));
            lines.push(String::new());
            lines.push((self.theme.hint)(self.hint()));
            return lines;
        }
        let start = self
            .selected
            .saturating_sub(self.max_visible / 2)
            .min(self.filtered_items.len().saturating_sub(self.max_visible));
        let end = (start + self.max_visible).min(self.filtered_items.len());
        let label_width = self
            .items
            .iter()
            .map(|i| visible_width(&i.label))
            .max()
            .unwrap_or(0)
            .min(30);
        for (index, item) in self.filtered_items[start..end].iter().enumerate() {
            let selected = start + index == self.selected;
            let prefix = if selected { &self.theme.cursor } else { "  " };
            let padded = format!(
                "{}{}",
                item.label,
                " ".repeat(label_width.saturating_sub(visible_width(&item.label)))
            );
            let label = (self.theme.label)(&padded, selected);
            let used = visible_width(prefix) + label_width + 2;
            let value = (self.theme.value)(
                &truncate_to_width(
                    &item.current_value,
                    width.saturating_sub(used + 2),
                    "",
                    false,
                ),
                selected,
            );
            lines.push(truncate_to_width(
                &format!("{prefix}{label}  {value}"),
                width,
                "",
                false,
            ));
        }
        if start > 0 || end < self.filtered_items.len() {
            lines.push((self.theme.hint)(&truncate_to_width(
                &format!("  ({}/{})", self.selected + 1, self.filtered_items.len()),
                width.saturating_sub(2),
                "",
                false,
            )));
        }
        if let Some(description) = self
            .filtered_items
            .get(self.selected)
            .and_then(|i| i.description.as_deref())
        {
            lines.push(String::new());
            for line in wrap_text_with_ansi(description, width.saturating_sub(4)) {
                lines.push((self.theme.description)(&format!("  {line}")));
            }
        }
        lines.push(String::new());
        lines.push(truncate_to_width(
            &(self.theme.hint)(self.hint()),
            width,
            "",
            false,
        ));
        lines
    }
    fn handle_input(&mut self, data: &str) {
        let kb = get_keybindings().lock().unwrap();
        let up = kb.matches(data, "tui.select.up");
        let down = kb.matches(data, "tui.select.down");
        let confirm = kb.matches(data, "tui.select.confirm");
        let cancel = kb.matches(data, "tui.select.cancel");
        let backspace = kb.matches(data, "tui.editor.deleteCharBackward");
        drop(kb);
        if up && !self.filtered_items.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.filtered_items.len() - 1)
        } else if down && !self.filtered_items.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_items.len()
        } else if confirm || data == " " {
            self.activate()
        } else if cancel {
            if let Some(callback) = &mut self.on_cancel {
                callback()
            }
        } else if self.search_enabled {
            if backspace {
                self.search.pop();
            } else if !data.chars().any(|c| c.is_control() || c == ' ') {
                self.search.push_str(data)
            }
            self.apply_filter()
        }
    }
}
