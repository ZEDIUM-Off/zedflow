//! ADK's Checkpointer adapter. SQL, content encoding and index consistency remain
//! in storage; this layer preserves the real ADK document and publishes receipts.
use crate::event_sink::EventSink;
use adk_graph::{checkpoint::Checkpointer, error::GraphError, state::Checkpoint};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::broadcast;
use zf_storage::{
    content_store::ContentStore,
    contracts::{CheckpointHeader, CheckpointStore},
};

#[derive(Clone)]
pub struct StoredCheckpointer {
    checkpoints: CheckpointStore,
    notices: broadcast::Sender<CheckpointHeader>,
    sender: Option<EventSink>,
}
impl StoredCheckpointer {
    pub fn new(checkpoints: CheckpointStore) -> Self {
        let (notices, _) = broadcast::channel(256);
        Self {
            checkpoints,
            notices,
            sender: None,
        }
    }
    pub fn storage(&self) -> &CheckpointStore {
        &self.checkpoints
    }
    pub fn content(&self) -> &ContentStore {
        self.checkpoints.content()
    }
    pub fn with_sender(mut self, sender: EventSink) -> Self {
        self.sender = Some(sender);
        self
    }
    pub fn subscribe(&self) -> broadcast::Receiver<CheckpointHeader> {
        self.notices.subscribe()
    }
    async fn save_exact(&self, checkpoint: &Checkpoint) -> Result<String> {
        let value = serde_json::to_value(checkpoint)?;
        // save returns only after the header and its entire immutable closure are
        // committed. No consumer is notified on a failed or conflicting save.
        let header = self.checkpoints.save(&value).await?;
        let _ = self.notices.send(header.clone());
        if let Some(sender) = &self.sender {
            let mut event = serde_json::to_value(header)?;
            event["type"] = json!("checkpoint_committed");
            let _ = sender.send(event).await;
        }
        Ok(checkpoint.checkpoint_id.clone())
    }
}
fn graph_error(error: impl Into<anyhow::Error>) -> GraphError {
    GraphError::CheckpointError(format!("{:#}", error.into()))
}
#[async_trait]
impl Checkpointer for StoredCheckpointer {
    async fn save(&self, checkpoint: &Checkpoint) -> adk_graph::error::Result<String> {
        self.save_exact(checkpoint).await.map_err(graph_error)
    }
    async fn load(&self, thread: &str) -> adk_graph::error::Result<Option<Checkpoint>> {
        self.checkpoints
            .load(thread)
            .await
            .map_err(graph_error)?
            .map(serde_json::from_value)
            .transpose()
            .map_err(graph_error)
    }
    async fn load_by_id(&self, id: &str) -> adk_graph::error::Result<Option<Checkpoint>> {
        self.checkpoints
            .load_by_id(id)
            .await
            .map_err(graph_error)?
            .map(serde_json::from_value)
            .transpose()
            .map_err(graph_error)
    }
    async fn list(&self, thread: &str) -> adk_graph::error::Result<Vec<Checkpoint>> {
        let headers = self
            .checkpoints
            .list_headers(thread)
            .await
            .map_err(graph_error)?;
        let mut result = Vec::with_capacity(headers.len());
        for header in headers {
            let value = self
                .checkpoints
                .hydrate(&header)
                .await
                .map_err(graph_error)?;
            result.push(serde_json::from_value(value).map_err(graph_error)?);
        }
        Ok(result)
    }
    async fn delete(&self, thread: &str) -> adk_graph::error::Result<()> {
        self.checkpoints.delete(thread).await.map_err(graph_error)
    }
    // Keep ADK's default no-prune policy. No implicit content garbage collection.
}
