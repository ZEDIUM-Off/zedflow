//! Committed projections and a common, replayable protocol for every transport.
use crate::{content_store::ContentStore, session_store};
use anyhow::Result;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, watch};

#[derive(Clone)]
pub struct SessionSync {
    pub shutdown: tokio_util::sync::CancellationToken,
    db: SqlitePool,
    inspector: Arc<dyn crate::contracts::RuntimeInspection>,
    content: ContentStore,
    runs: Arc<RwLock<HashMap<String, (Value, i64)>>>,
    changed: watch::Sender<u64>,
    metrics: Arc<RwLock<HashMap<String, Vec<Value>>>>,
}
impl SessionSync {
    pub fn new(
        db: SqlitePool,
        content: ContentStore,
        inspector: Arc<dyn crate::contracts::RuntimeInspection>,
    ) -> Self {
        let (changed, _) = watch::channel(0);
        Self {
            shutdown: tokio_util::sync::CancellationToken::new(),
            db,
            inspector,
            content,
            runs: Arc::new(RwLock::new(HashMap::new())),
            changed,
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn record_metrics(&self, id: &str, value: Value) {
        let mut metrics = self.metrics.write().await;
        let entries = metrics.entry(id.into()).or_default();
        if entries.len() >= 4096 {
            entries.drain(..1024);
        }
        entries.push(value);
    }
    pub async fn metrics(&self, id: &str) -> Value {
        json!({"batches":self.metrics.read().await.get(id).cloned().unwrap_or_default()})
    }
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
    pub async fn seed(&self, id: &str) -> Result<()> {
        let run = session_store::load_projection(&self.db, id).await?;
        let revision: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(seq),0) FROM events WHERE run=?")
                .bind(id)
                .fetch_one(&self.db)
                .await?;
        self.committed(id, run, revision).await;
        Ok(())
    }
    pub async fn head(&self, id: &str) -> Result<(Value, i64)> {
        if let Some((run, revision)) = self.runs.read().await.get(id) {
            return Ok((run["workspaceId"].clone(), *revision));
        }
        self.seed(id).await?;
        let runs = self.runs.read().await;
        let (run, revision) = runs.get(id).expect("seeded run");
        Ok((run["workspaceId"].clone(), *revision))
    }
    pub async fn latest(&self, id: &str) -> Result<(Value, i64)> {
        if let Some(value) = self.runs.read().await.get(id) {
            return Ok(value.clone());
        }
        self.seed(id).await?;
        Ok(self.runs.read().await.get(id).expect("seeded run").clone())
    }
    pub async fn committed(&self, id: &str, run: Value, revision: i64) {
        self.runs.write().await.insert(id.into(), (run, revision));
        self.changed
            .send_modify(|value| *value = value.wrapping_add(1));
    }
    pub async fn changes(&self, previous: &Value, next: &Value) -> Result<Value> {
        let mut operations = changes(previous, next);
        if previous["contextRef"] != next["contextRef"]
            && let Some(reference) = next["contextRef"].as_str()
        {
            let context = context_summary(self.content.resolve(reference).await?);
            operations
                .as_array_mut()
                .expect("operations array")
                .push(json!({"collection":"meta","value":{"context":context}}));
        }
        if previous["runtimeGraphRef"] != next["runtimeGraphRef"]
            && let Some(reference) = next["runtimeGraphRef"].as_str()
        {
            let summary = self
                .inspector
                .summary(&self.content.resolve(reference).await?)?;
            operations
                .as_array_mut()
                .expect("operations array")
                .push(json!({"collection":"meta","value":{"runtimeGraphSummary":summary}}));
        }
        Ok(operations)
    }
    pub async fn wire_run(&self, mut run: Value) -> Result<Value> {
        if let Some(reference) = run["compositionRef"].as_str() {
            run["composition"] = self.content.resolve(reference).await?;
        }
        if let Some(reference) = run["contextRef"].as_str() {
            run["context"] = context_summary(self.content.resolve(reference).await?);
        }
        if let Some(reference) = run["runtimeGraphRef"].as_str() {
            let summary = self
                .inspector
                .summary(&self.content.resolve(reference).await?)?;
            run["runtimeGraphSummary"] = summary;
        }
        run["hasFlowSource"] =
            json!(run["flowSourceRef"].is_string() || run["flowSource"].is_string());
        for key in [
            "flowSource",
            "flowPackage",
            "state",
            "input",
            "runtimeGraph",
        ] {
            run.as_object_mut().map(|value| value.remove(key));
        }
        run["state"] = json!({});
        run["messages"] = json!([]);
        for snapshot in run["contextSnapshots"].as_array_mut().into_iter().flatten() {
            *snapshot = session_store::context_index(snapshot);
        }
        if let Some(timeline) = run["timeline"].as_array() {
            let (entries, more, before) = timeline_page(timeline);
            run["timeline"] = json!(entries);
            run["timelineHasMore"] = json!(more);
            run["timelineBefore"] = json!(before);
        }
        crate::projection_summary::run(&mut run);
        Ok(run)
    }
    pub async fn payload(&self, id: &str, after: Option<i64>) -> Result<Value> {
        let (workspace, revision) = self.head(id).await?;
        if after == Some(revision) {
            return Ok(
                json!({"type":"heartbeat","runId":id,"workspaceId":workspace,"revision":revision,"cursor":revision}),
            );
        }
        if let Some(after) = after.filter(|cursor| *cursor > 0 && *cursor <= revision) {
            let rows: Vec<(i64,i64,String)> = sqlx::query_as("SELECT seq,base_revision,document FROM run_changes WHERE run=? AND seq>? ORDER BY seq LIMIT 64").bind(id).bind(after).fetch_all(&self.db).await?;
            if rows.first().is_some_and(|(_, base, _)| *base == after) {
                let mut operations = Vec::new();
                let mut cursor = after;
                for (next, base, raw) in rows {
                    if base != cursor {
                        break;
                    }
                    let doc: Value = serde_json::from_str(&raw)?;
                    let decoded = self
                        .content
                        .resolve(
                            doc["valueRef"]
                                .as_str()
                                .ok_or_else(|| anyhow::anyhow!("change reference missing"))?,
                        )
                        .await?;
                    operations.extend(
                        decoded
                            .as_array()
                            .into_iter()
                            .flatten()
                            .cloned()
                            .map(wire_operation),
                    );
                    cursor = next;
                }
                return Ok(
                    json!({"type":"delta","runId":id,"workspaceId":workspace,"baseRevision":after,"revision":cursor,"cursor":cursor,"ops":operations}),
                );
            }
        }
        let (run, revision) = self.latest(id).await?;
        Ok(
            json!({"type":"bootstrap","run":self.wire_run(run).await?,"revision":revision,"cursor":revision}),
        )
    }
}

pub fn changes(previous: &Value, next: &Value) -> Value {
    let mut operations = Vec::new();
    let mut meta = json!({});
    for (key, value) in next.as_object().into_iter().flatten() {
        if (key != "queue" && session_store::COLLECTIONS.contains(&key.as_str()))
            || [
                "composition",
                "context",
                "flowSource",
                "flowPackage",
                "state",
                "input",
                "runtimeGraph",
            ]
            .contains(&key.as_str())
        {
            continue;
        }
        if previous.get(key) != Some(value) {
            meta[key] = value.clone();
        }
    }
    if !meta.as_object().is_none_or(|v| v.is_empty()) {
        operations.push(json!({"collection":"meta","value":meta}));
    }
    for collection in [
        "timeline",
        "activities",
        "toolActivities",
        "contextSnapshots",
    ] {
        let before: HashMap<_, _> = previous[collection]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, v)| (session_store::entity_id(collection, v, i), v))
            .collect();
        for (index, value) in next[collection]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = session_store::entity_id(collection, value, index);
            if before.get(&id).is_none_or(|old| *old != value) {
                let value = if collection == "contextSnapshots" {
                    session_store::context_index(value)
                } else {
                    value.clone()
                };
                operations.push(json!({"collection":collection,"id":id,"value":value}));
            }
        }
    }
    json!(operations)
}

fn wire_operation(mut operation: Value) -> Value {
    if operation["collection"] == "contextSnapshots" {
        operation["value"] = session_store::context_index(&operation["value"]);
    }
    crate::projection_summary::operation(&mut operation);
    operation
}

/// A sequence can contain several visible messages. Never split that boundary,
/// otherwise a subsequent `before` cursor would skip its earlier siblings.
pub fn timeline_page(timeline: &[Value]) -> (Vec<Value>, bool, Option<i64>) {
    let mut start = timeline.len().saturating_sub(100);
    while start > 0 && timeline[start - 1]["seq"] == timeline[start]["seq"] {
        start -= 1;
    }
    let before = if start > 0 {
        timeline[start]["seq"].as_i64()
    } else {
        None
    };
    (timeline[start..].to_vec(), start > 0, before)
}

fn context_summary(mut context: Value) -> Value {
    for collection in ["instructions", "loadedSkills"] {
        for source in context[collection].as_array_mut().into_iter().flatten() {
            if let Some(object) = source.as_object_mut() {
                object.remove("content");
                object.remove("body");
            }
        }
    }
    context
}
