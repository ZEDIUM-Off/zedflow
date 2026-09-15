//! Storage contracts consumed by runtime adapters. The full checkpoint is opaque
//! JSON here: ADK owns deserialization and execution, storage preserves every field.
use crate::content_store::{ContentStore, PreparedContent};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;

/// A durable publication receipt. Its shape matches the historical checkpoint
/// header; runtime publishes a notification only after save returns successfully.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointHeader {
    pub checkpoint_id: String,
    pub thread_id: String,
    pub checkpoint_ref: String,
    pub state_ref: String,
    pub step: usize,
    pub pending_nodes: Vec<String>,
    pub consumed_messages: Vec<String>,
    pub created_at: String,
}

#[derive(Clone)]
pub struct CheckpointStore {
    content: ContentStore,
}
impl CheckpointStore {
    pub async fn new(content: ContentStore) -> Result<Self> {
        sqlx::query("CREATE TABLE IF NOT EXISTS zf_checkpoints(seq INTEGER PRIMARY KEY AUTOINCREMENT,checkpoint_id TEXT NOT NULL UNIQUE,thread_id TEXT NOT NULL,created_at TEXT NOT NULL,header TEXT NOT NULL)").execute(content.pool()).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS zf_checkpoints_thread ON zf_checkpoints(thread_id,created_at,seq)").execute(content.pool()).await?;
        Ok(Self { content })
    }
    pub fn content(&self) -> &ContentStore {
        &self.content
    }
    fn header(&self, value: &Value, prepared: &PreparedContent) -> Result<CheckpointHeader> {
        let state = value["state"]
            .as_object()
            .context("checkpoint state must be an object")?;
        let text = |field: &str| -> Result<String> {
            Ok(value[field]
                .as_str()
                .with_context(|| format!("checkpoint {field} missing"))?
                .into())
        };
        let state_ref = match prepared.object_field_reference("state") {
            Some(reference) => reference,
            None => self.content.prepare(&value["state"])?.reference,
        };
        Ok(CheckpointHeader {
            checkpoint_id: text("checkpoint_id")?,
            thread_id: text("thread_id")?,
            step: usize::try_from(value["step"].as_u64().context("invalid checkpoint step")?)?,
            pending_nodes: serde_json::from_value(value["pending_nodes"].clone())?,
            consumed_messages: state
                .get("__zedflow:consumedMessages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            created_at: text("created_at")?,
            checkpoint_ref: prepared.reference.clone(),
            state_ref,
        })
    }
    /// Commit the entire content closure and header together. A returned receipt
    /// is durable; no transport, observer or model is called by this operation.
    pub async fn save(&self, value: &Value) -> Result<CheckpointHeader> {
        let prepared = self.content.prepare(value)?;
        let header = self.header(value, &prepared)?;
        let mut tx = self.content.pool().begin().await?;
        self.content.persist_in(&mut tx, &prepared).await?;
        let prior: Option<String> =
            sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE checkpoint_id=?")
                .bind(&header.checkpoint_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(prior) = prior {
            ensure!(
                serde_json::from_str::<CheckpointHeader>(&prior)? == header,
                "checkpoint identity reused with different content"
            );
        } else {
            sqlx::query("INSERT INTO zf_checkpoints(checkpoint_id,thread_id,created_at,header) VALUES(?,?,?,?)")
                .bind(&header.checkpoint_id).bind(&header.thread_id).bind(&header.created_at)
                .bind(serde_json::to_string(&header)?).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.content.mark_committed(&prepared);
        Ok(header)
    }
    pub async fn hydrate(&self, header: &CheckpointHeader) -> Result<Value> {
        let value = self.content.resolve(&header.checkpoint_ref).await?;
        let prepared = self.content.prepare(&value)?;
        ensure!(
            self.header(&value, &prepared)? == *header,
            "checkpoint header/content mismatch"
        );
        Ok(value)
    }
    pub async fn latest_header(&self, thread: &str) -> Result<Option<CheckpointHeader>> {
        let raw: Option<String> = sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE thread_id=? ORDER BY created_at DESC,seq DESC LIMIT 1")
            .bind(thread).fetch_optional(self.content.pool()).await?;
        raw.map(|text| serde_json::from_str(&text).map_err(Into::into))
            .transpose()
    }
    pub async fn list_headers(&self, thread: &str) -> Result<Vec<CheckpointHeader>> {
        sqlx::query("SELECT header FROM zf_checkpoints WHERE thread_id=? ORDER BY created_at,seq")
            .bind(thread)
            .fetch_all(self.content.pool())
            .await?
            .into_iter()
            .map(|row| Ok(serde_json::from_str(row.try_get("header")?)?))
            .collect()
    }
    pub async fn list_run_headers(&self, run: &str) -> Result<Vec<CheckpointHeader>> {
        sqlx::query("SELECT header FROM zf_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ORDER BY created_at,seq")
            .bind(run).bind(run).bind(run).fetch_all(self.content.pool()).await?.into_iter()
            .map(|row| Ok(serde_json::from_str(row.try_get("header")?)?)).collect()
    }
    pub async fn load(&self, thread: &str) -> Result<Option<Value>> {
        match self.latest_header(thread).await? {
            Some(header) => Ok(Some(self.hydrate(&header).await?)),
            None => Ok(None),
        }
    }
    pub async fn load_by_id(&self, id: &str) -> Result<Option<Value>> {
        let raw: Option<String> =
            sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE checkpoint_id=?")
                .bind(id)
                .fetch_optional(self.content.pool())
                .await?;
        match raw {
            Some(raw) => Ok(Some(self.hydrate(&serde_json::from_str(&raw)?).await?)),
            None => Ok(None),
        }
    }
    /// Content must already be imported and hash-checked. Validate small index
    /// fields without expanding a potentially large checkpoint state.
    pub async fn install_headers(&self, headers: &[CheckpointHeader]) -> Result<()> {
        let roots: Vec<_> = headers
            .iter()
            .flat_map(|h| [h.checkpoint_ref.clone(), h.state_ref.clone()])
            .collect();
        let blobs = self.content.export_blobs(&roots).await?;
        let by_ref: std::collections::HashMap<_, _> = blobs
            .iter()
            .map(|b| (b.reference.as_str(), &b.body))
            .collect();
        for header in headers {
            let body = by_ref
                .get(header.checkpoint_ref.as_str())
                .context("missing checkpoint manifest")?;
            ensure!(
                body["kind"] == "object" && body["entries"]["state"] == header.state_ref,
                "checkpoint state reference mismatch"
            );
            let state = by_ref
                .get(header.state_ref.as_str())
                .context("missing checkpoint state")?;
            ensure!(
                state["kind"] == "object",
                "checkpoint state must be an object"
            );
            let consumed = match state["entries"]["__zedflow:consumedMessages"].as_str() {
                Some(reference) => self.content.resolve(reference).await?,
                None => Value::Null,
            };
            let messages: Vec<_> = consumed
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            ensure!(
                messages == header.consumed_messages,
                "checkpoint consumed messages mismatch"
            );
            for (field, expected) in [
                ("checkpoint_id", serde_json::json!(header.checkpoint_id)),
                ("thread_id", serde_json::json!(header.thread_id)),
                ("step", serde_json::json!(header.step)),
                ("pending_nodes", serde_json::json!(header.pending_nodes)),
                ("created_at", serde_json::json!(header.created_at)),
            ] {
                let id = body["entries"][field]
                    .as_str()
                    .context("checkpoint field missing")?;
                ensure!(
                    self.content.resolve(id).await? == expected,
                    "checkpoint header mismatch: {field}"
                );
            }
        }
        let mut tx = self.content.pool().begin().await?;
        for header in headers {
            sqlx::query("INSERT INTO zf_checkpoints(checkpoint_id,thread_id,created_at,header) VALUES(?,?,?,?)")
                .bind(&header.checkpoint_id).bind(&header.thread_id).bind(&header.created_at)
                .bind(serde_json::to_string(header)?).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn delete(&self, thread: &str) -> Result<()> {
        sqlx::query("DELETE FROM zf_checkpoints WHERE thread_id=?")
            .bind(thread)
            .execute(self.content.pool())
            .await?;
        Ok(())
    }
}

/// Interprets captured runtime definitions for read projections. The concrete
/// adapter belongs to runtime; storage never resolves flows or executes them.
/// An invalid definition must return an error, never a guessed summary.
pub trait RuntimeInspection: Send + Sync {
    fn summary(&self, definition: &Value) -> Result<Value>;
    fn interactive(&self, definition: &Value) -> Result<bool>;
}

/// Runtime-owned validation for historical checkpoint documents. Storage preserves
/// their complete values but cannot establish ADK compatibility on its own.
pub trait CheckpointCodec: Send + Sync {
    /// Decode the historical SQL projection into the runtime's canonical serialized
    /// checkpoint. ADK owns timestamp normalization and omitted default fields.
    fn decode_legacy_checkpoint(&self, checkpoint: Value) -> Result<Value>;
    fn validate_checkpoint(&self, checkpoint: &Value) -> Result<()>;
}

/// Borrowed captured inputs for a runtime receipt proof. These are archive values,
/// never current workspace data; proving resumability must not execute an effect.
pub struct ArchiveSnapshot<'a> {
    pub session_id: &'a str,
    pub run: &'a Value,
    pub records: &'a [crate::content_store::ContentRecord],
    pub registry: &'a crate::data_archive::RegistryArchive,
}

pub struct DependencyInspection {
    pub blocked: Vec<String>,
    pub resources: Vec<zf_core::types::Diagnostic>,
}

/// Runtime-owned interpretation of executable archive data. No permissive default
/// is provided: an exporter/importer must explicitly supply the running runtime's
/// checkpoint, source, dependency and sealed-receipt validation.
pub trait ArchiveRuntime: CheckpointCodec {
    fn definition_diagnostics(&self, run: &Value) -> Vec<String>;
    fn dependencies(&self, run: &Value) -> DependencyInspection;
    fn resumable_internal_receipt<'a>(
        &'a self,
        store: &'a ContentStore,
        snapshot: ArchiveSnapshot<'a>,
        receipt: &'a Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>>;
}
