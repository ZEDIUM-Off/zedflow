use crate::{utils::truncate_to_width, Component};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

pub struct SelectList {
    pub items: Vec<SelectItem>,
    pub selected: usize,
    pub max_visible: usize,
    pub on_select: Option<Box<dyn FnMut(&SelectItem)>>,
    pub on_cancel: Option<Box<dyn FnMut()>>,
}

impl SelectList {
    pub fn new(items: Vec<SelectItem>, max_visible: usize) -> Self {
        Self {
            items,
            selected: 0,
            max_visible,
            on_select: None,
            on_cancel: None,
        }
    }

    pub fn set_filter(&mut self, filter: &str) {
        self.items.retain(|item| {
            item.value
                .to_lowercase()
                .starts_with(&filter.to_lowercase())
        });
        self.selected = 0;
    }

    pub fn selected(&self) -> Option<&SelectItem> {
        self.items.get(self.selected)
    }
}

impl Component for SelectList {
    fn render(&self, width: usize) -> Vec<String> {
        if self.items.is_empty() {
            return vec!["  No matching commands".to_string()];
        }
        let start = self
            .selected
            .saturating_sub(self.max_visible / 2)
            .min(self.items.len().saturating_sub(self.max_visible));
        let end = (start + self.max_visible).min(self.items.len());
        self.items[start..end]
            .iter()
            .enumerate()
            .map(|(offset, item)| {
                let prefix = if start + offset == self.selected {
                    "→ "
                } else {
                    "  "
                };
                truncate_to_width(&format!("{prefix}{}", item.label), width, "", false)
            })
            .collect()
    }

    fn handle_input(&mut self, data: &str) {
        if self.items.is_empty() {
            return;
        }
        match data {
            "\x1b[A" => self.selected = self.selected.checked_sub(1).unwrap_or(self.items.len() - 1),
            "\x1b[B" => self.selected = (self.selected + 1) % self.items.len(),
            "\r" | "\n" => {
                if let Some(item) = self.items.get(self.selected).cloned() {
                    if let Some(on_select) = &mut self.on_select {
                        on_select(&item);
                    }
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
