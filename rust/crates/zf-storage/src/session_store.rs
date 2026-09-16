//! Session projections. Large immutable values live in the shared content store.
use crate::content_store::ContentStore;
use anyhow::{Context, Result};
use serde_json::{Value, json};
use sqlx::SqlitePool;

pub const COLLECTIONS: &[&str] = &[
    "activities",
    "timeline",
    "messages",
    "toolActivities",
    "contextSnapshots",
    "queue",
];
const ROOT_VALUES: &[&str] = &[
    "state",
    "context",
    "composition",
    "flowSource",
    "flowPackage",
    "input",
    "runtimeGraph",
];

pub async fn initialize(db: &SqlitePool) -> Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS run_entities(run TEXT NOT NULL, collection TEXT NOT NULL, id TEXT NOT NULL, position INTEGER NOT NULL, document TEXT NOT NULL, PRIMARY KEY(run,collection,id))").execute(db).await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS run_entities_order ON run_entities(run,collection,position)",
    )
    .execute(db)
    .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS events_run_seq ON events(run,seq)")
        .execute(db)
        .await?;
    sqlx::query("CREATE TABLE IF NOT EXISTS run_changes(seq INTEGER PRIMARY KEY,run TEXT NOT NULL,base_revision INTEGER NOT NULL,document TEXT NOT NULL)").execute(db).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS run_changes_order ON run_changes(run,seq)")
        .execute(db)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS runs_workspace_activity ON runs(json_extract(document,'$.workspaceId'),json_extract(document,'$.updatedAt'))").execute(db).await?;
    Ok(())
}

pub fn entity_id(collection: &str, item: &Value, index: usize) -> String {
    let key = match collection {
        "activities" => "occurrenceId",
        "toolActivities" => "callId",
        "contextSnapshots" => "invocationId",
        _ => "id",
    };
    item[key]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("legacy:{index}"))
}

/// Invocation identity is separate from its frozen context. In particular, the
/// skill catalogue must not be copied into every index entry or notification.
pub fn context_index(snapshot: &Value) -> Value {
    if !snapshot["contentRef"].is_string() {
        return snapshot.clone();
    }
    let mut index = json!({});
    for key in [
        "invocationId",
        "origin",
        "nodePath",
        "agentPath",
        "contentRef",
        "requestRef",
        "rawRef",
        "requestBoundary",
        "requestStatus",
    ] {
        if let Some(value) = snapshot.get(key) {
            index[key] = value.clone();
        }
    }
    index
}

async fn externalize(store: &ContentStore, value: &mut Value, field: &str) -> Result<()> {
    if let Some(content) = value.as_object_mut().and_then(|v| v.remove(field)) {
        let key = format!("{field}Ref");
        value[&key] = json!(store.intern(&content).await?);
    }
    Ok(())
}
async fn materialize(store: &ContentStore, value: &mut Value, field: &str) -> Result<()> {
    let key = format!("{field}Ref");
    if let Some(reference) = value[&key].as_str() {
        value[field] = store.resolve(reference).await?;
        value
            .as_object_mut()
            .context("object expected")?
            .remove(&key);
    }
    Ok(())
}

pub async fn compact_activity(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut result = value.clone();
    for field in ["input", "output"] {
        if result[format!("{field}Ref")].is_string() {
            result.as_object_mut().map(|v| v.remove(field));
        }
    }
    for field in ["input", "output"] {
        externalize(store, &mut result, field).await?;
    }
    Ok(result)
}
pub async fn hydrate_activity(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut result = value.clone();
    for field in ["input", "output", "state"] {
        materialize(store, &mut result, field).await?;
    }
    Ok(result)
}
pub async fn compact_tool(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut tool = value.clone();
    if !tool["argumentsRef"].is_string()
        && let Some(arguments) = tool.get("arguments")
    {
        let mut preview = json!({});
        for key in ["path", "file_path", "command", "cmd"] {
            if let Some(text) = arguments[key].as_str() {
                preview[key] = json!(text.chars().take(240).collect::<String>());
            }
        }
        tool["argumentsPreview"] = preview;
    }
    for field in ["arguments", "result", "output"] {
        externalize(store, &mut tool, field).await?;
    }
    Ok(tool)
}
pub async fn hydrate_tool(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut tool = value.clone();
    for field in ["arguments", "result", "output"] {
        materialize(store, &mut tool, field).await?;
    }
    tool.as_object_mut().map(|v| v.remove("argumentsPreview"));
    Ok(tool)
}
/// Restore just the active tool preview when an older producer sends fragments.
/// Complete previews never require a read, and unrelated entities remain untouched.
pub async fn restore_context(store: &ContentStore, run: &mut Value) -> Result<()> {
    if !run["context"].is_object() {
        materialize(store, run, "context").await?;
    }
    Ok(())
}
pub async fn restore_progress(store: &ContentStore, run: &mut Value, event: &Value) -> Result<()> {
    if event["type"] != "tool_progress" || event["content"].is_string() {
        return Ok(());
    }
    let Some(id) = event["callId"].as_str().or_else(|| event["id"].as_str()) else {
        return Ok(());
    };
    for tool in run["toolActivities"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .filter(|v| v["callId"] == id)
    {
        materialize(store, tool, "output").await?;
    }
    for entry in run["timeline"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .filter(|v| v["kind"] == "tool" && v["activity"]["callId"] == id)
    {
        materialize(store, &mut entry["activity"], "output").await?;
    }
    Ok(())
}
pub async fn compact_run(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut run = value.clone();
    for field in ROOT_VALUES {
        externalize(store, &mut run, field).await?;
    }
    for activity in run["activities"].as_array_mut().into_iter().flatten() {
        *activity = compact_activity(store, activity).await?;
    }
    for snapshot in run["contextSnapshots"].as_array_mut().into_iter().flatten() {
        if !snapshot["contentRef"].is_string() {
            snapshot["contentRef"] = json!(store.intern(snapshot).await?);
        }
        *snapshot = context_index(snapshot);
    }
    for tool in run["toolActivities"].as_array_mut().into_iter().flatten() {
        *tool = compact_tool(store, tool).await?;
    }
    for entry in run["timeline"].as_array_mut().into_iter().flatten() {
        if entry["kind"] == "tool" {
            entry["activity"] = compact_tool(store, &entry["activity"]).await?;
        }
    }
    run["storageVersion"] = json!(2);
    Ok(run)
}
/// Commands need the execution roots, not previously completed trace bodies.
pub async fn hydrate_command(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut run = value.clone();
    for field in ROOT_VALUES {
        materialize(store, &mut run, field).await?;
    }
    Ok(run)
}

pub async fn hydrate_run(store: &ContentStore, value: &Value) -> Result<Value> {
    let mut run = value.clone();
    for field in ROOT_VALUES {
        materialize(store, &mut run, field).await?;
    }
    for activity in run["activities"].as_array_mut().into_iter().flatten() {
        *activity = hydrate_activity(store, activity).await?;
    }
    for snapshot in run["contextSnapshots"].as_array_mut().into_iter().flatten() {
        if let Some(reference) = snapshot["contentRef"].as_str() {
            let index = context_index(snapshot);
            let mut captured = store.resolve(reference).await?;
            for (key, value) in index.as_object().into_iter().flatten() {
                captured[key] = value.clone();
            }
            *snapshot = captured;
        }
    }
    for tool in run["toolActivities"].as_array_mut().into_iter().flatten() {
        *tool = hydrate_tool(store, tool).await?;
    }
    for entry in run["timeline"].as_array_mut().into_iter().flatten() {
        if entry["kind"] == "tool" {
            entry["activity"] = hydrate_tool(store, &entry["activity"]).await?;
        }
    }
    Ok(run)
}

pub async fn load_projection(db: &SqlitePool, id: &str) -> Result<Value> {
    let raw: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
        .bind(id)
        .fetch_one(db)
        .await?;
    let mut run: Value = serde_json::from_str(&raw)?;
    if run["storageVersion"] != 2 {
        return Ok(run);
    }
    for collection in COLLECTIONS {
        run[*collection] = json!([]);
    }
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT collection,document FROM run_entities WHERE run=? ORDER BY collection,position",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let store = ContentStore::from_pool(db.clone());
    for (collection, raw) in rows {
        let entry: Value = serde_json::from_str(&raw)?;
        let mut entry = if let Some(reference) = entry["valueRef"].as_str() {
            store.resolve(reference).await?
        } else {
            entry
        };
        if collection == "contextSnapshots" {
            entry = context_index(&entry);
        }
        run[&collection]
            .as_array_mut()
            .context("unknown collection")?
            .push(entry);
    }
    Ok(run)
}
pub async fn load(db: &SqlitePool, id: &str) -> Result<Value> {
    let run = load_projection(db, id).await?;
    if run["storageVersion"] != 2 {
        return Ok(run);
    }
    hydrate_run(&ContentStore::from_pool(db.clone()), &run).await
}

pub async fn compact_event(store: &ContentStore, event: &Value) -> Result<Value> {
    let mut event = event.clone();
    for field in ["input", "output"] {
        if event[format!("{field}Ref")].is_string() {
            event.as_object_mut().map(|v| v.remove(field));
        }
    }
    for field in [
        "input",
        "output",
        "snapshot",
        "state",
        "result",
        "arguments",
        "content",
        "text",
        "chunk",
        "record",
        "data",
        "updates",
    ] {
        externalize(store, &mut event, field).await?;
    }
    Ok(event)
}
pub async fn hydrate_event(store: &ContentStore, event: &Value) -> Result<Value> {
    let mut event = event.clone();
    for field in [
        "input",
        "output",
        "snapshot",
        "state",
        "result",
        "arguments",
        "content",
        "text",
        "chunk",
        "record",
        "data",
        "updates",
    ] {
        materialize(store, &mut event, field).await?;
    }
    if let Some(reference) = event["snapshot"]["contentRef"].as_str() {
        event["snapshot"] = store.resolve(reference).await?;
    }
    Ok(event)
}

/// Prepared writes contain only immutable references; hashing happens before the transaction.
pub struct PreparedRun {
    pub metadata: Value,
    pub entities: Vec<(String, String, usize, String)>,
}
pub async fn prepare(store: &ContentStore, run: &Value) -> Result<PreparedRun> {
    prepare_delta(store, run, &json!({})).await
}
pub async fn prepare_delta(
    store: &ContentStore,
    run: &Value,
    previous: &Value,
) -> Result<PreparedRun> {
    let mut metadata = compact_run(store, run).await?;
    let mut entities = Vec::new();
    for collection in COLLECTIONS {
        if let Some(Value::Array(items)) =
            metadata.as_object_mut().and_then(|v| v.remove(*collection))
        {
            let before: std::collections::HashMap<_, _> = previous[*collection]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(i, v)| (entity_id(collection, v, i), (i, v)))
                .collect();
            for (index, item) in items.iter().enumerate() {
                let key = entity_id(collection, item, index);
                if before
                    .get(&key)
                    .is_some_and(|(i, value)| *i == index && *value == item)
                {
                    continue;
                }
                let reference = store.intern(item).await?;
                entities.push((
                    (*collection).into(),
                    key,
                    index,
                    json!({"valueRef":reference}).to_string(),
                ));
            }
        }
    }
    Ok(PreparedRun { metadata, entities })
}
pub async fn write(
    connection: &mut sqlx::SqliteConnection,
    id: &str,
    prepared: &PreparedRun,
) -> Result<()> {
    sqlx::query("INSERT INTO runs(id,document) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document").bind(id).bind(prepared.metadata.to_string()).execute(&mut *connection).await?;
    for (collection, key, position, document) in &prepared.entities {
        sqlx::query("INSERT INTO run_entities(run,collection,id,position,document) VALUES(?,?,?,?,?) ON CONFLICT(run,collection,id) DO UPDATE SET position=excluded.position,document=excluded.document WHERE document<>excluded.document OR position<>excluded.position")
            .bind(id).bind(collection).bind(key).bind(*position as i64).bind(document).execute(&mut *connection).await?;
    }
    Ok(())
}
pub async fn save(db: &SqlitePool, id: &str, run: &Value) -> Result<()> {
    let store = ContentStore::from_pool(db.clone());
    let prepared = prepare(&store, run).await?;
    let mut tx = db.begin().await?;
    write(&mut tx, id, &prepared).await?;
    tx.commit().await?;
    Ok(())
}

pub fn summary(run: &Value) -> Value {
    let mut result = json!({});
    for key in [
        "id",
        "name",
        "workspaceId",
        "workspacePath",
        "status",
        "createdAt",
        "updatedAt",
        "flowRef",
        "error",
        "interactive",
    ] {
        if let Some(value) = run.get(key) {
            result[key] = value.clone();
        }
    }
    result
}

/// One-time metadata upgrade. Only the small flow definition is materialized;
/// immutable histories and entity rows remain untouched.
pub async fn backfill_interactive(
    db: &SqlitePool,
    inspector: &dyn crate::contracts::RuntimeInspection,
) -> Result<usize> {
    let store = ContentStore::from_pool(db.clone());
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id,document FROM runs WHERE json_type(document,'$.interactive') IS NULL OR json_type(document,'$.interactive')='null' OR (json_extract(document,'$.interactive')=0 AND (json_type(document,'$.runtimeGraphRef')='text' OR json_type(document,'$.runtimeGraph')='object'))"
    ).fetch_all(db).await?;
    let mut count = 0;
    for (id, raw) in rows {
        let run: Value = serde_json::from_str(&raw)?;
        let runtime = if let Some(reference) = run["runtimeGraphRef"].as_str() {
            store.resolve(reference).await?
        } else {
            run["runtimeGraph"].clone()
        };
        let interactive = if !runtime.is_null() {
            inspector.interactive(&runtime)?
        } else {
            let composition = if let Some(reference) = run["compositionRef"].as_str() {
                store.resolve(reference).await?
            } else {
                run["composition"].clone()
            };
            // Incomplete legacy records stay visible until their definition is
            // available; absence is not evidence of autonomous behavior.
            if !composition["nodes"].is_array() {
                continue;
            }
            composition_interactive(&composition)
        };
        if run["interactive"].as_bool() == Some(interactive) {
            continue;
        }
        count += sqlx::query("UPDATE runs SET document=json_set(document,'$.interactive',json(?)) WHERE id=? AND (json_type(document,'$.interactive') IS NULL OR json_type(document,'$.interactive')='null' OR json_extract(document,'$.interactive')=0)")
            .bind(if interactive {"true"} else {"false"}).bind(id).execute(db).await?.rows_affected() as usize;
    }
    Ok(count)
}

/// Public flow contracts opt into interaction; legacy input/inbox are explicit waits.
pub fn composition_interactive(doc: &Value) -> bool {
    let nodes = doc["nodes"].as_array();
    if let Some(exports) = nodes
        .into_iter()
        .flatten()
        .find(|node| node["data"]["kind"] == "start")
        .and_then(|node| node["data"]["config"].get("exports"))
    {
        return exports["interactive"].as_bool().unwrap_or(false);
    }
    nodes.into_iter().flatten().any(|node| {
        matches!(node["data"]["kind"].as_str(), Some("input" | "inbox"))
            || node["data"]["kind"] == "subgraph"
                && composition_interactive(&node["data"]["config"]["composition"])
    })
}
