//! Typed interfaces for explicitly installed resource readers.
//! Host adapters own I/O, cancellation, content interning and native decoding.
use anyhow::{Context, Result, ensure};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use zf_core::types::{DataType, TypeRegistry, compatible, validate_type, validate_value};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReaderInput {
    Literal {
        value: Value,
    },
    State {
        field: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pointer: Option<String>,
    },
}
impl ReaderInput {
    pub fn validate(&self) -> Result<()> {
        if let Self::State { field, pointer } = self {
            ensure!(
                !field.is_empty()
                    && pointer
                        .as_ref()
                        .is_none_or(|p| p.is_empty() || p.starts_with('/')),
                "Invalid state selector for reader input"
            );
        }
        Ok(())
    }
    pub fn capture(&self, state: &HashMap<String, Value>) -> Result<Option<Value>> {
        self.validate()?;
        Ok(match self {
            Self::Literal { value } => Some(value.clone()),
            Self::State { field, pointer } => state
                .get(field)
                .and_then(|v| pointer.as_ref().map_or(Some(v), |p| v.pointer(p)))
                .cloned(),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ReaderOutput {
    Fixed {
        data_type: DataType,
    },
    /// A JSON decoder instantiates the resource's explicit declared type. It
    /// validates JSON against that type and never coerces strings or numbers.
    DeclaredJson,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReaderContract {
    pub id: String,
    pub version: String,
    pub input: DataType,
    pub output: ReaderOutput,
}

#[derive(Debug)]
pub struct ReadResource {
    pub value: Arc<Value>,
    pub provenance: Value,
}
/// Implementations are explicitly linked by the native host. Loading a Rust
/// strategy or source package does not execute or register native reader code.
pub trait ResourceReader: Send + Sync {
    fn contract(&self) -> ReaderContract;
    fn read<'a>(&'a self, input: &'a Value) -> BoxFuture<'a, Result<Option<ReadResource>>>;
}
#[derive(Default)]
pub struct ReaderRegistry {
    readers: BTreeMap<String, Arc<dyn ResourceReader>>,
}
impl ReaderRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, reader: Arc<dyn ResourceReader>) -> Result<()> {
        let contract = reader.contract();
        ensure!(
            !contract.id.is_empty() && !contract.version.is_empty(),
            "Reader identity/version must be explicit"
        );
        ensure!(
            !self.readers.contains_key(&contract.id),
            "Reader identity already registered: {}",
            contract.id
        );
        self.readers.insert(contract.id, reader);
        Ok(())
    }
    pub fn contains(&self, id: &str) -> bool {
        self.readers.contains_key(id)
    }
    pub fn contracts(&self) -> Vec<ReaderContract> {
        self.readers.values().map(|r| r.contract()).collect()
    }
    pub async fn read(
        &self,
        id: &str,
        input: &Value,
        expected: &DataType,
        types: &TypeRegistry,
    ) -> Result<Option<ReadResource>> {
        let reader = self
            .readers
            .get(id)
            .with_context(|| format!("Native resource reader is not installed: {id}"))?;
        let contract = reader.contract();
        validate_type(expected, types).map_err(|d| anyhow::anyhow!("{d:?}"))?;
        validate_value(&contract.input, input, types)
            .map_err(|d| anyhow::anyhow!("Reader {id} input: {d:?}"))?;
        if let ReaderOutput::Fixed { data_type } = &contract.output {
            ensure!(
                compatible(data_type, expected, types),
                "Reader {id} output contract differs from the declared resource type"
            );
        }
        let result = reader.read(input).await?;
        let Some(mut result) = result else {
            return Ok(None);
        };
        validate_value(expected, &result.value, types)
            .map_err(|d| anyhow::anyhow!("Reader {id} output: {d:?}"))?;
        result.provenance =
            json!({"kind":"reader", "reader":contract, "input":input, "source":result.provenance});
        Ok(Some(result))
    }
}

#[derive(Clone, Copy)]
enum Builtin {
    FileText,
    FileJson,
    SqliteJson,
    ContentText,
    ContentJson,
}
fn record(names: &[&str]) -> DataType {
    DataType::Record {
        fields: names
            .iter()
            .map(|name| ((*name).into(), DataType::Text))
            .collect(),
    }
}
impl Builtin {
    fn contract(&self) -> ReaderContract {
        let (id, input, output) = match self {
            Self::FileText => (
                "file.text",
                record(&["path"]),
                ReaderOutput::Fixed {
                    data_type: DataType::Text,
                },
            ),
            Self::FileJson => ("file.json", record(&["path"]), ReaderOutput::DeclaredJson),
            Self::SqliteJson => (
                "sqlite.json",
                record(&["path", "table", "id"]),
                ReaderOutput::DeclaredJson,
            ),
            Self::ContentText => (
                "content.text",
                record(&["contentRef"]),
                ReaderOutput::Fixed {
                    data_type: DataType::Text,
                },
            ),
            Self::ContentJson => (
                "content.json",
                record(&["contentRef"]),
                ReaderOutput::DeclaredJson,
            ),
        };
        ReaderContract {
            id: id.into(),
            version: "1".into(),
            input,
            output,
        }
    }
}
/// Available native contracts for authoring and export inspection only.
/// Querying this catalogue never installs a reader or opens its resource.
pub fn standard_contracts() -> Vec<ReaderContract> {
    let mut contracts: Vec<_> = [
        Builtin::FileText,
        Builtin::FileJson,
        Builtin::SqliteJson,
        Builtin::ContentText,
        Builtin::ContentJson,
    ]
    .iter()
    .map(Builtin::contract)
    .collect();
    contracts.sort_by(|a, b| a.id.cmp(&b.id));
    contracts
}
