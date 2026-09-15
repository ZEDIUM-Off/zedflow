//! Runtime-owned interpretation of frozen archive and checkpoint documents.
use adk_graph::state::Checkpoint;
use anyhow::Result;
use serde_json::Value;
use zf_storage::contracts::CheckpointCodec;

/// Uses the exact installed ADK schema rather than duplicating its defaults and
/// serialization rules in the SQLite reader. No node is executed by this codec.
pub struct AdkCheckpointCodec;
impl CheckpointCodec for AdkCheckpointCodec {
    fn decode_legacy_checkpoint(&self, value: Value) -> Result<Value> {
        let checkpoint: Checkpoint = serde_json::from_value(value)?;
        Ok(serde_json::to_value(checkpoint)?)
    }
    fn validate_checkpoint(&self, value: &Value) -> Result<()> {
        let _: Checkpoint = serde_json::from_value(value.clone())?;
        Ok(())
    }
}
