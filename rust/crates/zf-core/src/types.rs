//! Shared resource contracts for composition and context strategies.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DataType {
    Boolean,
    Number,
    Text,
    List {
        item: Box<DataType>,
    },
    Record {
        fields: BTreeMap<String, DataType>,
    },
    /// An immutable media reference, not inline base64 or a promise of adapter support.
    Media {
        media_type: String,
    },
    Named {
        name: String,
    },
}

pub type TypeRegistry = BTreeMap<String, DataType>;

pub use crate::diagnostics::Diagnostic;

/// Named data types have nominal identity; records allow additional source fields.
/// There is no string/number coercion or implicit media conversion.
pub fn compatible(source: &DataType, target: &DataType, types: &TypeRegistry) -> bool {
    if validate_type(source, types).is_err() || validate_type(target, types).is_err() {
        return false;
    }
    fn accepts(source: &DataType, target: &DataType) -> bool {
        match (source, target) {
            (DataType::Record { fields: source }, DataType::Record { fields: target }) => target
                .iter()
                .all(|(key, value)| source.get(key).is_some_and(|s| accepts(s, value))),
            (DataType::List { item: source }, DataType::List { item: target }) => {
                accepts(source, target)
            }
            _ => source == target,
        }
    }
    accepts(source, target)
}

pub fn validate_type(data_type: &DataType, types: &TypeRegistry) -> Result<(), Vec<Diagnostic>> {
    let mut errors = Vec::new();
    check_type(
        data_type,
        types,
        "$",
        &mut BTreeSet::new(),
        0,
        &mut 0,
        &mut errors,
    );
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_type(
    data_type: &DataType,
    types: &TypeRegistry,
    path: &str,
    visiting: &mut BTreeSet<String>,
    depth: usize,
    visited: &mut usize,
    errors: &mut Vec<Diagnostic>,
) {
    // A small registry can describe an exponentially expanded DAG of named
    // records. Depth alone does not bound validation work.
    if *visited >= 4096 {
        if *visited == 4096 {
            errors.push(Diagnostic::new(
                "type_complexity",
                path,
                "Type expansion exceeds 4096 nodes",
            ));
            *visited += 1;
        }
        return;
    }
    *visited += 1;
    if depth > 64 {
        errors.push(Diagnostic::new(
            "type_depth",
            path,
            "Type nesting exceeds 64 levels",
        ));
        return;
    }
    match data_type {
        DataType::Named { name } => {
            if name.trim().is_empty() {
                errors.push(Diagnostic::new(
                    "type_name",
                    path,
                    "Named type must have an identity",
                ));
                return;
            }
            if !visiting.insert(name.clone()) {
                errors.push(Diagnostic::new(
                    "type_cycle",
                    path,
                    format!("Recursive type: {name}"),
                ));
            } else {
                match types.get(name) {
                    Some(value) => {
                        check_type(value, types, path, visiting, depth + 1, visited, errors)
                    }
                    None => errors.push(Diagnostic::new(
                        "unknown_type",
                        path,
                        format!("Unknown type: {name}"),
                    )),
                }
                visiting.remove(name);
            }
        }
        DataType::Record { fields } => {
            for (name, field) in fields {
                check_type(
                    field,
                    types,
                    &format!("{path}.{name}"),
                    visiting,
                    depth + 1,
                    visited,
                    errors,
                );
                if *visited > 4096 {
                    break;
                }
            }
        }
        DataType::List { item } => check_type(
            item,
            types,
            &format!("{path}[]"),
            visiting,
            depth + 1,
            visited,
            errors,
        ),
        DataType::Media { media_type } if media_type.trim().is_empty() => {
            errors.push(Diagnostic::new(
                "media_type",
                path,
                "Media type must be specified",
            ));
        }
        _ => {}
    }
}

pub fn validate_value(
    data_type: &DataType,
    value: &Value,
    types: &TypeRegistry,
) -> Result<(), Vec<Diagnostic>> {
    validate_type(data_type, types)?;
    let mut errors = Vec::new();
    check_value(data_type, value, types, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_value(
    data_type: &DataType,
    value: &Value,
    types: &TypeRegistry,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let matches = match data_type {
        DataType::Boolean => value.is_boolean(),
        DataType::Number => value.is_number(),
        DataType::Text => value.is_string(),
        DataType::Named { name } => {
            // Type validation above establishes that the reference exists and is acyclic.
            if let Some(resolved) = types.get(name) {
                check_value(resolved, value, types, path, errors);
            }
            true
        }
        DataType::Media { media_type } => {
            value.is_object()
                && value
                    .get("contentRef")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty())
                && value.get("mediaType").and_then(Value::as_str) == Some(media_type.as_str())
        }
        DataType::List { item } => {
            if let Some(values) = value.as_array() {
                for (index, value) in values.iter().enumerate() {
                    check_value(item, value, types, &format!("{path}[{index}]"), errors);
                }
                true
            } else {
                false
            }
        }
        DataType::Record { fields } => {
            if let Some(values) = value.as_object() {
                for (name, data_type) in fields {
                    let field_path = format!("{path}.{name}");
                    if let Some(value) = values.get(name) {
                        check_value(data_type, value, types, &field_path, errors);
                    } else {
                        errors.push(Diagnostic::new(
                            "missing_field",
                            field_path,
                            "Required field is absent",
                        ));
                    }
                }
                true
            } else {
                false
            }
        }
    };
    if !matches {
        errors.push(Diagnostic::new(
            "value_type",
            path,
            format!("Value does not match {data_type:?}"),
        ));
    }
}
