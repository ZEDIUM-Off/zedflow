//! Ordered enable/disable state for Ctrl+P scoped models.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedModel {
    pub full_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedModelsSelector {
    all: Vec<ScopedModel>,
    enabled: Option<Vec<String>>,
    filtered: Vec<usize>,
    pub selected: usize,
}

impl ScopedModelsSelector {
    #[must_use]
    pub fn new(all: Vec<ScopedModel>, enabled: Option<Vec<String>>) -> Self {
        let filtered = (0..all.len()).collect();
        Self {
            all,
            enabled,
            filtered,
            selected: 0,
        }
    }

    #[must_use]
    pub fn enabled_ids(&self) -> Option<&[String]> {
        self.enabled.as_deref()
    }
    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled
            .as_ref()
            .is_none_or(|ids| ids.iter().any(|item| item == id))
    }

    pub fn filter(&mut self, query: &str) {
        let query = query.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .enumerate()
            .filter_map(|(i, model)| {
                (query.is_empty()
                    || model.full_id.to_lowercase().contains(&query)
                    || model.name.to_lowercase().contains(&query))
                .then_some(i)
            })
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn toggle_selected(&mut self) {
        let Some(&index) = self.filtered.get(self.selected) else {
            return;
        };
        let id = self.all[index].full_id.clone();
        match &mut self.enabled {
            None => self.enabled = Some(vec![id]),
            Some(ids) if ids.iter().any(|item| item == &id) => ids.retain(|item| item != &id),
            Some(ids) => ids.push(id),
        }
    }

    pub fn enable_all(&mut self) {
        self.enabled = None;
    }
    pub fn clear_all(&mut self) {
        self.enabled = Some(Vec::new());
    }

    pub fn move_enabled(&mut self, delta: isize) {
        let Some(&model_index) = self.filtered.get(self.selected) else {
            return;
        };
        let id = &self.all[model_index].full_id;
        let Some(ids) = &mut self.enabled else {
            return;
        };
        let Some(index) = ids.iter().position(|item| item == id) else {
            return;
        };
        let next = index
            .saturating_add_signed(delta)
            .min(ids.len().saturating_sub(1));
        ids.swap(index, next);
    }

    #[must_use]
    pub fn selected_model(&self) -> Option<&ScopedModel> {
        self.filtered.get(self.selected).map(|&i| &self.all[i])
    }
}
