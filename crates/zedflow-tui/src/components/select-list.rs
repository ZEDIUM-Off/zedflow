use std::sync::Arc;

use crate::{
    Component,
    keybindings::get_keybindings,
    utils::{truncate_to_width, visible_width},
};

const DEFAULT_PRIMARY_COLUMN_WIDTH: usize = 32;
const PRIMARY_COLUMN_GAP: usize = 2;
const MIN_DESCRIPTION_WIDTH: usize = 10;

pub type SelectListStyle = Arc<dyn Fn(&str) -> String + Send + Sync>;
pub type SelectListPrimaryTruncator =
    Arc<dyn for<'a> Fn(SelectListTruncatePrimaryContext<'a>) -> String + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Clone)]
pub struct SelectListTheme {
    pub selected_prefix: SelectListStyle,
    pub selected_text: SelectListStyle,
    pub description: SelectListStyle,
    pub scroll_info: SelectListStyle,
    pub no_match: SelectListStyle,
}

pub struct SelectListTruncatePrimaryContext<'a> {
    pub text: &'a str,
    pub max_width: usize,
    pub column_width: usize,
    pub item: &'a SelectItem,
    pub is_selected: bool,
}

#[derive(Clone, Default)]
pub struct SelectListLayoutOptions {
    pub min_primary_column_width: Option<usize>,
    pub max_primary_column_width: Option<usize>,
    pub truncate_primary: Option<SelectListPrimaryTruncator>,
}

pub struct SelectList {
    items: Vec<SelectItem>,
    filtered_items: Vec<SelectItem>,
    pub selected: usize,
    pub max_visible: usize,
    theme: SelectListTheme,
    layout: SelectListLayoutOptions,
    pub on_select: Option<Box<dyn FnMut(&SelectItem)>>,
    pub on_cancel: Option<Box<dyn FnMut()>>,
    pub on_selection_change: Option<Box<dyn FnMut(&SelectItem)>>,
}

impl SelectList {
    pub fn new(items: Vec<SelectItem>, max_visible: usize, theme: SelectListTheme) -> Self {
        Self::with_layout(
            items,
            max_visible,
            theme,
            SelectListLayoutOptions::default(),
        )
    }

    pub fn with_layout(
        items: Vec<SelectItem>,
        max_visible: usize,
        theme: SelectListTheme,
        layout: SelectListLayoutOptions,
    ) -> Self {
        Self {
            filtered_items: items.clone(),
            items,
            selected: 0,
            max_visible,
            theme,
            layout,
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

    fn primary_column_width(&self) -> usize {
        let raw_min = self
            .layout
            .min_primary_column_width
            .or(self.layout.max_primary_column_width)
            .unwrap_or(DEFAULT_PRIMARY_COLUMN_WIDTH);
        let raw_max = self
            .layout
            .max_primary_column_width
            .or(self.layout.min_primary_column_width)
            .unwrap_or(DEFAULT_PRIMARY_COLUMN_WIDTH);
        let min = raw_min.min(raw_max).max(1);
        let max = raw_min.max(raw_max).max(1);
        let widest = self
            .filtered_items
            .iter()
            .map(|item| visible_width(display_value(item)) + PRIMARY_COLUMN_GAP)
            .max()
            .unwrap_or(0);
        widest.clamp(min, max)
    }

    fn truncate_primary(
        &self,
        item: &SelectItem,
        is_selected: bool,
        max_width: usize,
        column_width: usize,
    ) -> String {
        let text = display_value(item);
        let value = self.layout.truncate_primary.as_ref().map_or_else(
            || truncate_to_width(text, max_width, "", false),
            |truncate| {
                truncate(SelectListTruncatePrimaryContext {
                    text,
                    max_width,
                    column_width,
                    item,
                    is_selected,
                })
            },
        );
        truncate_to_width(&value, max_width, "", false)
    }

    fn render_item(
        &self,
        item: &SelectItem,
        is_selected: bool,
        width: usize,
        description: Option<&str>,
        primary_column_width: usize,
    ) -> String {
        let prefix = if is_selected { "→ " } else { "  " };
        let prefix_width = visible_width(prefix);

        if let Some(description) = description.filter(|_| width > 40) {
            let column_width = primary_column_width
                .min(width.saturating_sub(prefix_width + 4))
                .max(1);
            let max_primary_width = column_width.saturating_sub(PRIMARY_COLUMN_GAP).max(1);
            let primary = self.truncate_primary(item, is_selected, max_primary_width, column_width);
            let spacing = " ".repeat(column_width.saturating_sub(visible_width(&primary)).max(1));
            let remaining = width.saturating_sub(
                prefix_width + visible_width(&primary) + visible_width(&spacing) + 2,
            );
            if remaining > MIN_DESCRIPTION_WIDTH {
                let description = truncate_to_width(description, remaining, "", false);
                if is_selected {
                    return (self.theme.selected_text)(&format!(
                        "{prefix}{primary}{spacing}{description}"
                    ));
                }
                return format!(
                    "{prefix}{primary}{}",
                    (self.theme.description)(&format!("{spacing}{description}"))
                );
            }
        }

        let max_width = width.saturating_sub(prefix_width + 2).max(1);
        let primary = self.truncate_primary(item, is_selected, max_width, max_width);
        let line = format!("{prefix}{primary}");
        if is_selected {
            (self.theme.selected_text)(&line)
        } else {
            line
        }
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
            return vec![(self.theme.no_match)("  No matching commands")];
        }

        let column_width = self.primary_column_width();
        let start = self
            .selected
            .saturating_sub(self.max_visible / 2)
            .min(self.filtered_items.len().saturating_sub(self.max_visible));
        let end = (start + self.max_visible).min(self.filtered_items.len());
        let mut lines = self.filtered_items[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let description = item
                    .description
                    .as_deref()
                    .map(normalize_to_single_line)
                    .filter(|description| !description.is_empty());
                self.render_item(
                    item,
                    start + offset == self.selected,
                    width,
                    description.as_deref(),
                    column_width,
                )
            })
            .collect::<Vec<_>>();

        if start > 0 || end < self.filtered_items.len() {
            let text = truncate_to_width(
                &format!("  ({}/{})", self.selected + 1, self.filtered_items.len()),
                width.saturating_sub(2),
                "",
                false,
            );
            lines.push((self.theme.scroll_info)(&text));
        }
        lines
    }

    fn handle_input(&mut self, data: &str) {
        let keybindings = get_keybindings().lock().unwrap();
        let up = keybindings.matches(data, "tui.select.up");
        let down = keybindings.matches(data, "tui.select.down");
        let confirm = keybindings.matches(data, "tui.select.confirm");
        let cancel = keybindings.matches(data, "tui.select.cancel");
        drop(keybindings);

        if up && !self.filtered_items.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.filtered_items.len() - 1);
            self.notify();
        } else if down && !self.filtered_items.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_items.len();
            self.notify();
        } else if confirm {
            if let (Some(item), Some(callback)) = (
                self.filtered_items.get(self.selected).cloned(),
                &mut self.on_select,
            ) {
                callback(&item);
            }
        } else if cancel {
            if let Some(callback) = &mut self.on_cancel {
                callback();
            }
        }
    }
}

fn display_value(item: &SelectItem) -> &str {
    if item.label.is_empty() {
        &item.value
    } else {
        &item.label
    }
}

fn normalize_to_single_line(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut in_line_break = false;
    for character in text.chars() {
        if matches!(character, '\r' | '\n') {
            if !in_line_break {
                normalized.push(' ');
                in_line_break = true;
            }
        } else {
            normalized.push(character);
            in_line_break = false;
        }
    }
    normalized.trim().to_string()
}
