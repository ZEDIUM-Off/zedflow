//! Explicit workspace bridge sources, using the same atomic file writer as strategies.
use crate::context_store::SourceStore;
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};
use zf_core::diagnostics::Diagnostic;
use zf_flows::{bridge_source, composition::BridgeDefinition};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeFile {
    pub key: String,
    pub path: PathBuf,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge: Option<BridgeDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone)]
pub struct BridgeStore {
    files: SourceStore,
}
impl BridgeStore {
    pub fn new(workspace: PathBuf) -> Result<Self> {
        Ok(Self {
            files: SourceStore::new(workspace, &["bridges"])?,
        })
    }
    pub async fn list(&self) -> Result<Vec<BridgeFile>> {
        Ok(self
            .files
            .list()
            .await?
            .into_iter()
            .map(|file| decode(file, false))
            .collect())
    }
    pub async fn read(&self, key: &str) -> Result<BridgeFile> {
        Ok(decode(self.files.read(key).await?, true))
    }
    pub async fn save(
        &self,
        key: &str,
        bridge: &BridgeDefinition,
        expected_hash: Option<&str>,
    ) -> Result<BridgeFile> {
        let source = bridge_source::generate(bridge).map_err(|d| anyhow::anyhow!("{d:?}"))?;
        let parsed = bridge_source::parse(&source).map_err(|d| anyhow::anyhow!("{d:?}"))?;
        ensure!(
            serde_json::to_value(parsed)? == serde_json::to_value(bridge)?,
            "Bridge source does not round-trip"
        );
        Ok(decode(
            self.files.save(key, &source, expected_hash).await?,
            true,
        ))
    }
    pub async fn catalog(&self) -> Result<BTreeMap<String, BridgeDefinition>> {
        let mut result = BTreeMap::new();
        for file in self.list().await? {
            if let Some(bridge) = file.bridge {
                result.insert(file.key, bridge);
            }
        }
        Ok(result)
    }
}
fn decode(file: super::context_store::SourceFile, include_source: bool) -> BridgeFile {
    let mut diagnostics = file.diagnostics;
    let bridge = file
        .source
        .as_deref()
        .and_then(|source| match bridge_source::parse(source) {
            Ok(value) => Some(value),
            Err(errors) => {
                diagnostics.extend(errors);
                None
            }
        });
    BridgeFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: if include_source { file.source } else { None },
        bridge,
        diagnostics,
    }
}
