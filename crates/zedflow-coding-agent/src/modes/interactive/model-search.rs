//! Search strings used by the model and scoped-model selectors.

#[derive(Debug, Clone, Copy)]
pub struct ModelSearchItem<'a> {
    pub id: &'a str,
    pub provider: &'a str,
    pub name: Option<&'a str>,
}

#[must_use]
pub fn model_search_text(item: ModelSearchItem<'_>) -> String {
    let name = item.name.map_or(String::new(), |name| format!(" {name}"));
    format!(
        "{} {} {}/{} {} {}{}",
        item.id, item.provider, item.provider, item.id, item.provider, item.id, name
    )
}

/// Keeps the provider first so exact `provider/model` queries outrank proxy IDs.
#[must_use]
pub fn model_selector_search_text(item: ModelSearchItem<'_>) -> String {
    let name = item.name.map_or(String::new(), |name| format!(" {name}"));
    format!(
        "{} {}/{} {} {}{}",
        item.provider, item.provider, item.id, item.provider, item.id, name
    )
}
