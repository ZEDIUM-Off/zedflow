//! Pure examples identified by their complete reachable schema.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use zf_core::types::{DataType, TypeRegistry, validate_value};

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}
pub(crate) fn diagnostics_error(errors: Vec<zf_core::diagnostics::Diagnostic>) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        errors
            .into_iter()
            .map(|e| format!("{}: {} [{}]", e.path, e.message, e.code))
            .collect::<Vec<_>>()
            .join("; ")
    )
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypeExample {
    pub version: u32,
    pub id: String,
    pub label: String,
    pub schema_hash: String,
    pub data_type: DataType,
    pub types: TypeRegistry,
    pub value: Value,
}

pub fn identity(data_type: &DataType, types: &TypeRegistry) -> Result<(String, TypeRegistry)> {
    zf_core::types::validate_type(data_type, types).map_err(diagnostics_error)?;
    let types = dependencies(data_type, types);
    let bytes = serde_json::to_vec(&(data_type, &types))?;
    Ok((hash(&bytes), types))
}

pub fn parse(source: &str) -> Result<TypeExample> {
    ensure!(source.len() <= 1024 * 1024, "Example exceeds 1 MiB");
    let example: TypeExample = serde_json::from_str(source)?;
    ensure!(example.version == 1, "Unsupported example version");
    ensure!(
        super::context::valid_id(&example.id) && example.id != "builtin",
        "Invalid workspace example identity"
    );
    ensure!(
        !example.label.trim().is_empty() && example.label.len() <= 200,
        "Invalid example label"
    );
    let (actual, _) = identity(&example.data_type, &example.types)?;
    ensure!(
        actual == example.schema_hash,
        "Example schema hash mismatch"
    );
    validate_value(&example.data_type, &example.value, &example.types)
        .map_err(diagnostics_error)?;
    Ok(example)
}

pub fn dependencies(data_type: &DataType, registry: &TypeRegistry) -> TypeRegistry {
    let mut result = TypeRegistry::new();
    let mut pending = vec![data_type];
    while let Some(data_type) = pending.pop() {
        match data_type {
            DataType::Named { name } => {
                if !result.contains_key(name)
                    && let Some(value) = registry.get(name)
                {
                    result.insert(name.clone(), value.clone());
                    pending.push(value);
                }
            }
            DataType::Record { fields } => pending.extend(fields.values()),
            DataType::List { item } => pending.push(item),
            _ => {}
        }
    }
    result
}

fn example_value(ty: &DataType, types: &TypeRegistry) -> Option<Value> {
    Some(match ty {
        DataType::Text => Value::String("Texte d’exemple".into()),
        DataType::Boolean => Value::Bool(true),
        DataType::Number => Value::from(42),
        DataType::List { item } => Value::Array(vec![example_value(item, types)?]),
        DataType::Record { fields } => Value::Object(
            fields
                .iter()
                .map(|(k, v)| Some((k.clone(), example_value(v, types)?)))
                .collect::<Option<_>>()?,
        ),
        DataType::Named { name } => example_value(types.get(name)?, types)?,
        DataType::Media { .. } => return None,
    })
}

/// A synthetic example for authoring; no catalogue or resource is activated.
pub fn builtin(data_type: &DataType, types: &TypeRegistry) -> Result<Option<TypeExample>> {
    let (schema_hash, types) = identity(data_type, types)?;
    Ok(example_value(data_type, &types).map(|value| TypeExample {
        version: 1,
        id: "builtin".into(),
        label: "Exemple du type".into(),
        schema_hash,
        data_type: data_type.clone(),
        types,
        value,
    }))
}
