use crate::{utils::truncate_to_width, Component};

#[derive(Clone, Debug)]
pub struct SettingItem {
    pub label: String,
    pub value: String,
}

pub struct SettingsList {
    pub items: Vec<SettingItem>,
}

impl SettingsList {
    pub fn new(items: Vec<SettingItem>) -> Self {
        Self { items }
    }
}

impl Component for SettingsList {
    fn render(&self, width: usize) -> Vec<String> {
        self.items
            .iter()
            .map(|item| truncate_to_width(&format!("{}: {}", item.label, item.value), width))
            .collect()
    }
}
