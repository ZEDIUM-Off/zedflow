//! Explicit, typed catalogues supplied by the caller of a context evaluation.
//! Functions are pure expression programs; no ambient resources or capabilities.
use super::context::ContextExpr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zf_core::types::DataType;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryKind {
    Projection,
    Subprogram,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextFunction {
    pub parameters: BTreeMap<String, DataType>,
    pub output: DataType,
    pub body: ContextExpr,
}
impl ContextFunction {
    pub fn new(
        parameters: BTreeMap<String, DataType>,
        output: DataType,
        body: ContextExpr,
    ) -> Self {
        Self {
            parameters,
            output,
            body,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextLibrary {
    #[serde(default)]
    pub projections: BTreeMap<String, ContextFunction>,
    #[serde(default)]
    pub subprograms: BTreeMap<String, ContextFunction>,
}

impl ContextLibrary {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn projection(mut self, name: &str, function: ContextFunction) -> Self {
        self.projections.insert(name.into(), function);
        self
    }
    pub fn subprogram(mut self, name: &str, function: ContextFunction) -> Self {
        self.subprograms.insert(name.into(), function);
        self
    }
    pub fn get(&self, kind: LibraryKind, name: &str) -> Option<&ContextFunction> {
        match kind {
            LibraryKind::Projection => self.projections.get(name),
            LibraryKind::Subprogram => self.subprograms.get(name),
        }
    }
}
