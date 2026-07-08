//! Generated provider model catalog ported from Pi.

use crate::models::Model;

/// Returns all generated models for this provider.
#[must_use]
pub fn ant_ling_models() -> Vec<Model> {
    vec![
        Model {
            provider: "ant-ling".into(),
            id: "Ling-2.6-1T".into(),
            api: "openai-completions".into(),
        }, // Ling 2.6 1T
        Model {
            provider: "ant-ling".into(),
            id: "Ling-2.6-flash".into(),
            api: "openai-completions".into(),
        }, // Ling 2.6 Flash
        Model {
            provider: "ant-ling".into(),
            id: "Ring-2.6-1T".into(),
            api: "openai-completions".into(),
        }, // Ring 2.6 1T
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_generated_models() {
        assert!(!ant_ling_models().is_empty());
    }
}
