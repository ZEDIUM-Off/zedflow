use crate::{fuzzy::fuzzy_filter, utils::truncate_to_width, Component};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub current_value: String,
    pub values: Vec<String>,
}

pub struct SettingsList {
    pub items: Vec<SettingItem>,
    filtered_items: Vec<SettingItem>,
    pub selected: usize,
    pub max_visible: usize,
    pub on_change: Option<Box<dyn FnMut(&str, &str)>>,
    pub on_cancel: Option<Box<dyn FnMut()>>,
}

impl SettingsList {
    pub fn new(items: Vec<SettingItem>, max_visible: usize) -> Self {
        Self {
            filtered_items: items.clone(),
            items,
            selected: 0,
            max_visible,
            on_change: None,
            on_cancel: None,
        }
    }

    pub fn update_value(&mut self, id: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(item) = self.items.iter_mut().find(|item| item.id == id) {
            item.current_value = value;
        }
        self.filtered_items = self.items.clone();
    }

    pub fn set_filter(&mut self, query: &str) {
        self.filtered_items = fuzzy_filter(&self.items, query, |item| &item.label);
        self.selected = 0;
    }
}

impl Component for SettingsList {
    fn render(&self, width: usize) -> Vec<String> {
        if self.filtered_items.is_empty() {
            return vec![truncate_to_width("  No matching settings", width, "", false)];
        }
        let start = self
            .selected
            .saturating_sub(self.max_visible / 2)
            .min(self.filtered_items.len().saturating_sub(self.max_visible));
        let end = (start + self.max_visible).min(self.filtered_items.len());
        self.filtered_items[start..end]
            .iter()
            .map(|item| truncate_to_width(&format!("{}: {}", item.label, item.current_value), width, "", false))
            .collect()
    }

    fn handle_input(&mut self, data: &str) {
        if self.filtered_items.is_empty() {
            return;
        }
        match data {
            "\x1b[A" => self.selected = self.selected.checked_sub(1).unwrap_or(self.filtered_items.len() - 1),
            "\x1b[B" => self.selected = (self.selected + 1) % self.filtered_items.len(),
            "\r" | "\n" | " " => {
                let Some(item) = self.filtered_items.get_mut(self.selected) else { return };
                if item.values.is_empty() {
                    return;
                }
                let index = item.values.iter().position(|v| v == &item.current_value).unwrap_or(0);
                item.current_value = item.values[(index + 1) % item.values.len()].clone();
                if let Some(source) = self.items.iter_mut().find(|source| source.id == item.id) {
                    source.current_value = item.current_value.clone();
                }
                if let Some(on_change) = &mut self.on_change {
                    on_change(&item.id, &item.current_value);
                }
            }
            "\x1b" => {
                if let Some(on_cancel) = &mut self.on_cancel {
                    on_cancel();
                }
            }
            _ => {}
        }
    }
}
