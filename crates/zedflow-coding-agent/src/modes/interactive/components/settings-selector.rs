//! Deterministic settings-list state. Persistence remains in `SettingsManager`.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsSelector {
    items: Vec<SettingChoice>,
    filtered: Vec<usize>,
    pub selected: usize,
}
impl SettingsSelector {
    #[must_use]
    pub fn new(items: Vec<SettingChoice>) -> Self {
        let filtered = (0..items.len()).collect();
        Self {
            items,
            filtered,
            selected: 0,
        }
    }
    pub fn filter(&mut self, query: &str) {
        let query = query.to_lowercase();
        self.filtered = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let text =
                    format!("{} {} {}", item.label, item.description, item.value).to_lowercase();
                text.contains(&query).then_some(index)
            })
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }
    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.filtered.len().saturating_sub(1));
    }
    pub fn activate(&mut self) -> Option<SettingsAction> {
        let index = *self.filtered.get(self.selected)?;
        let item = &mut self.items[index];
        if item.values.is_empty() {
            return None;
        }
        let current = item
            .values
            .iter()
            .position(|value| value == &item.value)
            .unwrap_or(0);
        item.value = item.values[(current + 1) % item.values.len()].clone();
        Some(SettingsAction::Change {
            id: item.id.clone(),
            value: item.value.clone(),
        })
    }
    #[must_use]
    pub const fn cancel(&self) -> SettingsAction {
        SettingsAction::Cancel
    }
    #[must_use]
    pub fn selected_item(&self) -> Option<&SettingChoice> {
        self.filtered
            .get(self.selected)
            .map(|index| &self.items[*index])
    }
}
