//! Searchable model-selector state, separated from persistence and rendering.

use crate::model_search::{ModelSearchItem, model_selector_search_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelItem {
    pub provider: String,
    pub id: String,
    pub name: String,
}

impl ModelItem {
    #[must_use]
    pub fn full_id(&self) -> String {
        format!("{}/{}", self.provider, self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelScope {
    All,
    Scoped,
}

#[derive(Debug, Clone)]
pub struct ModelSelector {
    all: Vec<ModelItem>,
    scoped: Vec<ModelItem>,
    filtered: Vec<ModelItem>,
    pub scope: ModelScope,
    pub selected: usize,
    pub error: Option<String>,
}

impl ModelSelector {
    #[must_use]
    pub fn new(mut all: Vec<ModelItem>, scoped_ids: &[String], current: Option<&str>) -> Self {
        all.sort_by_key(|model| {
            (
                current != Some(model.full_id().as_str()),
                model.provider.clone(),
            )
        });
        let scoped = scoped_ids
            .iter()
            .filter_map(|id| all.iter().find(|m| m.full_id() == *id).cloned())
            .collect::<Vec<_>>();
        let scope = if scoped.is_empty() {
            ModelScope::All
        } else {
            ModelScope::Scoped
        };
        let filtered = if scope == ModelScope::Scoped {
            scoped.clone()
        } else {
            all.clone()
        };
        let selected = current
            .and_then(|id| filtered.iter().position(|m| m.full_id() == id))
            .unwrap_or(0);
        Self {
            all,
            scoped,
            filtered,
            scope,
            selected,
            error: None,
        }
    }

    pub fn toggle_scope(&mut self, query: &str) {
        if self.scoped.is_empty() {
            return;
        }
        self.scope = if self.scope == ModelScope::All {
            ModelScope::Scoped
        } else {
            ModelScope::All
        };
        self.selected = 0;
        self.filter(query);
    }

    pub fn filter(&mut self, query: &str) {
        let active = if self.scope == ModelScope::Scoped {
            &self.scoped
        } else {
            &self.all
        };
        self.filtered = if query.is_empty() {
            active.clone()
        } else {
            active
                .iter()
                .filter(|model| {
                    fuzzy_matches(
                        &model_selector_search_text(ModelSearchItem {
                            id: &model.id,
                            provider: &model.provider,
                            name: Some(&model.name),
                        }),
                        query,
                    )
                })
                .cloned()
                .collect()
        };
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
    }

    #[must_use]
    pub fn selected_model(&self) -> Option<&ModelItem> {
        self.filtered.get(self.selected)
    }
    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.filtered.len()
    }
}

fn fuzzy_matches(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    query.to_lowercase().split_whitespace().all(|token| {
        let mut chars = token.chars();
        let mut next = chars.next();
        for character in text.chars() {
            if Some(character) == next {
                next = chars.next();
            }
        }
        next.is_none()
    })
}
