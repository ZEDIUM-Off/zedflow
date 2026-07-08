//! TypeBox helper schemas ported from Pi's `packages/ai/src/utils/typebox-helpers.ts`.

use serde_json::{Map, Value};

/// Options accepted by [`string_enum`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StringEnumOptions {
    /// Optional JSON Schema description. Empty strings are omitted, matching Pi's truthy spread.
    pub description: Option<String>,
    /// Optional JSON Schema default value. Empty strings are omitted, matching Pi's truthy spread.
    pub default: Option<String>,
}

/// Creates a string enum schema compatible with providers that do not support `anyOf`/`const` patterns.
#[must_use]
pub fn string_enum(values: &[impl AsRef<str>], options: Option<&StringEnumOptions>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".to_string(), Value::String("string".to_string()));
    schema.insert(
        "enum".to_string(),
        Value::Array(
            values
                .iter()
                .map(|value| Value::String(value.as_ref().to_string()))
                .collect(),
        ),
    );

    if let Some(description) = options
        .and_then(|options| options.description.as_deref())
        .filter(|description| !description.is_empty())
    {
        schema.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }

    if let Some(default) = options
        .and_then(|options| options.default.as_deref())
        .filter(|default| !default.is_empty())
    {
        schema.insert("default".to_string(), Value::String(default.to_string()));
    }

    Value::Object(schema)
}

#[cfg(test)]
mod tests {
    use super::{StringEnumOptions, string_enum};
    use serde_json::json;

    #[test]
    fn string_enum_matches_pi_schema_shape() {
        assert_eq!(
            string_enum(
                &["add", "subtract"],
                Some(&StringEnumOptions {
                    description: Some("operation".to_string()),
                    default: Some("add".to_string()),
                }),
            ),
            json!({
                "type": "string",
                "enum": ["add", "subtract"],
                "description": "operation",
                "default": "add",
            }),
        );

        assert_eq!(
            string_enum(
                &[""],
                Some(&StringEnumOptions {
                    description: Some(String::new()),
                    default: Some(String::new()),
                }),
            ),
            json!({"type": "string", "enum": [""]}),
        );
    }
}
