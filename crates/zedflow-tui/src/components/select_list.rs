use crate::{utils::truncate_to_width, Component};

#[derive(Clone, Debug)]
pub struct SelectItem {
    pub value: String,
    pub label: String,
}

pub struct SelectList {
    pub items: Vec<SelectItem>,
    pub selected: usize,
}

impl SelectList {
    pub fn new(items: Vec<SelectItem>) -> Self {
        Self { items, selected: 0 }
    }

    pub fn selected(&self) -> Option<&SelectItem> {
        self.items.get(self.selected)
    }
}

impl Component for SelectList {
    fn render(&self, width: usize) -> Vec<String> {
        self.items
            .iter()
            .map(|item| truncate_to_width(&item.label, width))
            .collect()
    }
}
