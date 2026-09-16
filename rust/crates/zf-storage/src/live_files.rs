//! Frozen inputs for authoring preflight. This module reads definition data only;
//! execution owns compatibility checks, composition and publication decisions.
use crate::content_store::ContentStore;
use anyhow::{Context, Result};
use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

/// Both the head record and its immutable target are preserved. The consuming
/// execution service validates the instance identity and executable definition.
pub struct DefinitionHead {
    pub head: Value,
    pub definition: Value,
}

/// Values captured at one SQLite read snapshot. Unrelated state, messages,
/// activities and tool outputs are neither fetched nor hydrated.
pub struct RunDefinitionSnapshot {
    pub run_id: String,
    pub flow_ref: Value,
    pub composition: Value,
    pub flow_source: Value,
    pub flow_package: Value,
    pub runtime_graph: Value,
    pub revision_heads: BTreeMap<String, DefinitionHead>,
    pub runtime_graph_head: Option<DefinitionHead>,
}

async fn field(store: &ContentStore, fields: &Value, name: &str) -> Result<Value> {
    if let Some(reference) = fields[format!("{name}Ref")].as_str() {
        store.resolve(reference).await
    } else {
        Ok(fields[name].clone())
    }
}

async fn head(store: &ContentStore, reference: &str, target: &str) -> Result<DefinitionHead> {
    let head = store.resolve(reference).await?;
    let definition = store
        .resolve(
            head[target]
                .as_str()
                .with_context(|| format!("definition head {target} absent"))?,
        )
        .await?;
    Ok(DefinitionHead { head, definition })
}

/// Select current non-completed runs of one workspace. All mutable references are
/// read in one transaction; immutable contents can then be resolved after commit.
/// The authoring service must still serialize writes and recheck publication
/// preconditions before accepting a proposal based on this snapshot.
pub async fn snapshots(db: &SqlitePool, workspace_id: &str) -> Result<Vec<RunDefinitionSnapshot>> {
    let mut tx = db.begin().await?;
    let rows: Vec<(String,String)> = sqlx::query_as(
        "SELECT id,json_object('flowRef',json_extract(document,'$.flowRef'),'composition',json_extract(document,'$.composition'),'compositionRef',json_extract(document,'$.compositionRef'),'flowSource',json_extract(document,'$.flowSource'),'flowSourceRef',json_extract(document,'$.flowSourceRef'),'flowPackage',json_extract(document,'$.flowPackage'),'flowPackageRef',json_extract(document,'$.flowPackageRef'),'runtimeGraph',json_extract(document,'$.runtimeGraph'),'runtimeGraphRef',json_extract(document,'$.runtimeGraphRef')) FROM runs WHERE json_extract(document,'$.workspaceId')=? AND COALESCE(json_extract(document,'$.status'),'')!='completed' ORDER BY id"
    ).bind(workspace_id).fetch_all(&mut *tx).await?;
    let mut captures = Vec::with_capacity(rows.len());
    for (id, raw) in rows {
        let references: Vec<(String,String,String)> = sqlx::query_as(
            "SELECT kind,key,value_ref FROM zf_records WHERE scope=? AND (kind='revision-heads' OR (kind='runtime-graph-heads' AND key='current')) ORDER BY kind,key"
        ).bind(&id).fetch_all(&mut *tx).await?;
        captures.push((id, raw, references));
    }
    tx.commit().await?;
    let store = ContentStore::from_pool(db.clone());
    let mut result = Vec::with_capacity(captures.len());
    for (run_id, raw, references) in captures {
        let fields: Value = serde_json::from_str(&raw)?;
        let mut revision_heads = BTreeMap::new();
        let mut runtime_graph_head = None;
        for (kind, key, reference) in references {
            if kind == "revision-heads" {
                revision_heads.insert(key, head(&store, &reference, "definitionRef").await?);
            } else {
                runtime_graph_head = Some(head(&store, &reference, "graphRef").await?);
            }
        }
        result.push(RunDefinitionSnapshot {
            run_id,
            composition: field(&store, &fields, "composition").await?,
            flow_source: field(&store, &fields, "flowSource").await?,
            flow_package: field(&store, &fields, "flowPackage").await?,
            runtime_graph: field(&store, &fields, "runtimeGraph").await?,
            flow_ref: fields["flowRef"].clone(),
            revision_heads,
            runtime_graph_head,
        });
    }
    Ok(result)
}
