use zedflow_ai::types::Model;

#[must_use]
pub fn filter_models<'a>(models: &'a [Model], pattern: Option<&str>) -> Vec<&'a Model> {
    let Some(pattern) = pattern.filter(|value| !value.is_empty()) else {
        return models.iter().collect();
    };
    let pattern = pattern.to_lowercase();
    models
        .iter()
        .filter(|model| {
            format!("{}/{} {}", model.provider, model.id, model.name)
                .to_lowercase()
                .contains(&pattern)
        })
        .collect()
}
