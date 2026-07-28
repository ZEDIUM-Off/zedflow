use crate::{
    Component,
    utils::{truncate_to_width, visible_width},
};

const PRIMARY_COLUMN_WIDTH: usize = 32;
const PRIMARY_COLUMN_GAP: usize = 2;
const MIN_DESCRIPTION_WIDTH: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

pub struct SelectList {
    items: Vec<SelectItem>,
    filtered_items: Vec<SelectItem>,
    pub selected: usize,
    pub max_visible: usize,
    pub on_select: Option<Box<dyn FnMut(&SelectItem)>>,
    pub on_cancel: Option<Box<dyn FnMut()>>,
    pub on_selection_change: Option<Box<dyn FnMut(&SelectItem)>>,
}

impl SelectList {
    pub fn new(items: Vec<SelectItem>, max_visible: usize) -> Self {
        Self {
            filtered_items: items.clone(),
            items,
            selected: 0,
            max_visible,
            on_select: None,
            on_cancel: None,
            on_selection_change: None,
        }
    }
    pub fn set_filter(&mut self, filter: &str) {
        let filter = filter.to_lowercase();
        self.filtered_items = self
            .items
            .iter()
            .filter(|item| item.value.to_lowercase().starts_with(&filter))
            .cloned()
            .collect();
        self.selected = 0;
    }
    pub fn set_selected_index(&mut self, index: usize) {
        self.selected = index.min(self.filtered_items.len().saturating_sub(1));
    }
    pub fn selected(&self) -> Option<&SelectItem> {
        self.filtered_items.get(self.selected)
    }
    fn notify(&mut self) {
        if let (Some(item), Some(callback)) = (
            self.filtered_items.get(self.selected).cloned(),
            &mut self.on_selection_change,
        ) {
            callback(&item);
        }
    }
}

impl Component for SelectList {
    fn render(&self, width: usize) -> Vec<String> {
        if self.filtered_items.is_empty() {
            return vec!["  No matching commands".to_string()];
        }
        let start = self
            .selected
            .saturating_sub(self.max_visible / 2)
            .min(self.filtered_items.len().saturating_sub(self.max_visible));
        let end = (start + self.max_visible).min(self.filtered_items.len());
        let widest = self
            .filtered_items
            .iter()
            .map(|i| {
                visible_width(if i.label.is_empty() {
                    &i.value
                } else {
                    &i.label
                }) + PRIMARY_COLUMN_GAP
            })
            .max()
            .unwrap_or(1);
        let column = widest.clamp(1, PRIMARY_COLUMN_WIDTH);
        let mut lines = self.filtered_items[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let prefix = if start + offset == self.selected {
                    "→ "
                } else {
                    "  "
                };
                let label = if item.label.is_empty() {
                    &item.value
                } else {
                    &item.label
                };
                let description = item
                    .description
                    .as_deref()
                    .map(|s| s.replace(['\r', '\n'], " ").trim().to_string())
                    .filter(|s| !s.is_empty());
                if let Some(description) = description.filter(|_| width > 40) {
                    let effective = column.min(width.saturating_sub(6)).max(1);
                    let primary = truncate_to_width(
                        label,
                        effective.saturating_sub(PRIMARY_COLUMN_GAP).max(1),
                        "",
                        false,
                    );
                    let spacing =
                        " ".repeat(effective.saturating_sub(visible_width(&primary)).max(1));
                    let remaining = width.saturating_sub(
                        visible_width(prefix) + visible_width(&primary) + spacing.len() + 2,
                    );
                    if remaining > MIN_DESCRIPTION_WIDTH {
                        return format!(
                            "{prefix}{primary}{spacing}{}",
                            truncate_to_width(&description, remaining, "", false)
                        );
                    }
                }
                truncate_to_width(
                    &format!("{prefix}{label}"),
                    width.saturating_sub(2),
                    "",
                    false,
                )
            })
            .collect::<Vec<_>>();
        if start > 0 || end < self.filtered_items.len() {
            lines.push(truncate_to_width(
                &format!("  ({}/{})", self.selected + 1, self.filtered_items.len()),
                width.saturating_sub(2),
                "",
                false,
            ));
        }
        lines
    }
    fn handle_input(&mut self, data: &str) {
        if self.filtered_items.is_empty() {
            if data == "\x1b" {
                if let Some(f) = &mut self.on_cancel {
                    f()
                }
            }
            return;
        }
        match data {
            "\x1b[A" => {
                self.selected = self
                    .selected
                    .checked_sub(1)
                    .unwrap_or(self.filtered_items.len() - 1);
                self.notify();
            }
            "\x1b[B" => {
                self.selected = (self.selected + 1) % self.filtered_items.len();
                self.notify();
            }
            "\r" | "\n" => {
                if let (Some(item), Some(f)) = (
                    self.filtered_items.get(self.selected).cloned(),
                    &mut self.on_select,
                ) {
                    f(&item)
                }
            }
            "\x1b" | "\x03" => {
                if let Some(f) = &mut self.on_cancel {
                    f()
                }
            }
            _ => {}
        }
    }
}
