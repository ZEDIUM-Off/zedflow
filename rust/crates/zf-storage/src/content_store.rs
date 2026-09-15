//! Immutable, content-addressed JSON. Arrays share every prefix; object values
//! are references too. The representation is storage-only, never flow state.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection, SqlitePool};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug)]
pub struct ContentStore {
    pool: SqlitePool,
    committed: Arc<RwLock<HashSet<String>>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentBlob {
    pub reference: String,
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRecord {
    pub scope: String,
    pub kind: String,
    pub key: String,
    pub value_ref: String,
}

pub(crate) struct PreparedContent {
    pub reference: String,
    pending: BTreeMap<String, Value>,
}

impl PreparedContent {
    pub fn object_field_reference(&self, field: &str) -> Option<String> {
        self.pending
            .get(&self.reference)?
            .get("entries")?
            .get(field)?
            .as_str()
            .map(str::to_owned)
    }
}

fn reference(body: &Value) -> Result<String> {
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(body)?)
    ))
}
fn add(body: Value, pending: &mut BTreeMap<String, Value>) -> Result<String> {
    let id = reference(&body)?;
    pending.entry(id.clone()).or_insert(body);
    Ok(id)
}
fn encode(value: &Value, pending: &mut BTreeMap<String, Value>, depth: usize) -> Result<String> {
    ensure!(depth <= 512, "JSON nesting exceeds content store limit");
    match value {
        Value::String(text) if text.len() > 4096 => {
            let mut head = add(json!({"kind":"array"}), pending)?;
            let mut remainder = text.as_str();
            while !remainder.is_empty() {
                let end = remainder.floor_char_boundary(remainder.len().min(4096));
                let (chunk, rest) = remainder.split_at(end);
                let item = add(json!({"kind":"scalar","value":chunk}), pending)?;
                head = add(
                    json!({"kind":"sequence","parent":head,"item":item}),
                    pending,
                )?;
                remainder = rest;
            }
            add(json!({"kind":"string","head":head}), pending)
        }
        Value::Array(items) => {
            let mut head = add(json!({"kind":"array"}), pending)?;
            for item in items {
                let item = encode(item, pending, depth + 1)?;
                head = add(
                    json!({"kind":"sequence","parent":head,"item":item}),
                    pending,
                )?;
            }
            Ok(head)
        }
        Value::Object(items) => {
            let mut entries = BTreeMap::new();
            for (key, value) in items {
                entries.insert(key, encode(value, pending, depth + 1)?);
            }
            add(json!({"kind":"object","entries":entries}), pending)
        }
        _ => add(json!({"kind":"scalar","value":value}), pending),
    }
}
fn dependencies(body: &Value) -> Result<Vec<String>> {
    let object = body.as_object().context("content body is not an object")?;
    let kind = body["kind"].as_str().context("missing content kind")?;
    let refs = match kind {
        "scalar" => {
            ensure!(
                object.len() == 2
                    && object.contains_key("value")
                    && !body["value"].is_array()
                    && !body["value"].is_object(),
                "invalid scalar content"
            );
            vec![]
        }
        "array" => {
            ensure!(object.len() == 1, "invalid empty array content");
            vec![]
        }
        "string" => {
            ensure!(object.len() == 2, "invalid chunked string content");
            vec![body["head"].as_str().context("invalid string head")?.into()]
        }
        "sequence" => {
            ensure!(object.len() == 3, "invalid sequence content");
            vec![
                body["parent"]
                    .as_str()
                    .context("invalid sequence parent")?
                    .into(),
                body["item"]
                    .as_str()
                    .context("invalid sequence item")?
                    .into(),
            ]
        }
        "object" => {
            ensure!(object.len() == 2, "invalid object content");
            body["entries"]
                .as_object()
                .context("invalid object entries")?
                .values()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .context("invalid object value reference")
                })
                .collect::<Result<Vec<_>>>()?
        }
        _ => bail!("unknown content kind: {kind}"),
    };
    for id in &refs {
        ensure!(
            id.len() == 71
                && id.starts_with("sha256:")
                && id[7..]
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "invalid content reference"
        );
    }
    Ok(refs)
}
fn decode(id: &str, blobs: &HashMap<String, Value>, depth: usize) -> Result<Value> {
    ensure!(depth <= 512, "JSON nesting exceeds content store limit");
    let body = blobs
        .get(id)
        .with_context(|| format!("missing content: {id}"))?;
    match body["kind"].as_str() {
        Some("scalar") => Ok(body["value"].clone()),
        Some("object") => {
            let mut object = serde_json::Map::new();
            for (key, value) in body["entries"].as_object().context("invalid object")? {
                object.insert(
                    key.clone(),
                    decode(
                        value.as_str().context("invalid reference")?,
                        blobs,
                        depth + 1,
                    )?,
                );
            }
            Ok(Value::Object(object))
        }
        Some("array" | "sequence") => Ok(Value::Array(
            sequence_refs(id, blobs)?
                .into_iter()
                .map(|item| decode(item, blobs, depth + 1))
                .collect::<Result<Vec<_>>>()?,
        )),
        Some("string") => {
            let mut text = String::new();
            for item in sequence_refs(body["head"].as_str().context("invalid string head")?, blobs)?
            {
                let chunk = blobs.get(item).context("missing string chunk")?;
                ensure!(chunk["kind"] == "scalar", "string chunk is not a scalar");
                text.push_str(
                    chunk["value"]
                        .as_str()
                        .context("string chunk is not text")?,
                );
            }
            Ok(Value::String(text))
        }
        _ => bail!("invalid content kind"),
    }
}
fn sequence_refs<'a>(id: &'a str, blobs: &'a HashMap<String, Value>) -> Result<Vec<&'a str>> {
    let mut items = Vec::new();
    let mut current = id;
    let mut visited = BTreeSet::new();
    loop {
        ensure!(visited.insert(current), "cyclic content sequence");
        let body = blobs.get(current).context("missing sequence content")?;
        match body["kind"].as_str() {
            Some("array") => break,
            Some("sequence") => {
                items.push(body["item"].as_str().context("invalid sequence item")?);
                current = body["parent"].as_str().context("invalid sequence parent")?;
            }
            _ => bail!("sequence parent is not an array"),
        }
    }
    items.reverse();
    Ok(items)
}

fn expanded_size_in(root: &str, blobs: &HashMap<String, Value>) -> Result<u64> {
    let mut sizes = HashMap::<String, u64>::new();
    let mut active = HashSet::new();
    let mut stack = vec![(root.to_owned(), false)];
    while let Some((id, finish)) = stack.pop() {
        if sizes.contains_key(&id) {
            continue;
        }
        let body = blobs
            .get(&id)
            .context("missing content while measuring expansion")?;
        if !finish {
            ensure!(active.insert(id.clone()), "cyclic content expansion");
            stack.push((id, true));
            for child in dependencies(body)? {
                if !sizes.contains_key(&child) {
                    stack.push((child, false));
                }
            }
            continue;
        }
        let child_size = |field: &str| -> Result<u64> {
            sizes
                .get(body[field].as_str().context("invalid child reference")?)
                .copied()
                .context("unmeasured content dependency")
        };
        let add = |left: u64, right: u64| {
            left.checked_add(right)
                .context("content expansion size overflow")
        };
        let size = match body["kind"].as_str() {
            Some("scalar") => u64::try_from(serde_json::to_vec(&body["value"])?.len())?,
            Some("array") => 2,
            Some("sequence") => add(add(child_size("parent")?, child_size("item")?)?, 1)?,
            Some("string") => child_size("head")?, // Conservative: includes chunk quotes/separators.
            Some("object") => {
                let mut total = 2;
                for (key, reference) in body["entries"].as_object().context("invalid object")? {
                    total = add(total, u64::try_from(serde_json::to_vec(key)?.len())?)?;
                    total = add(
                        total,
                        *sizes
                            .get(reference.as_str().context("invalid reference")?)
                            .context("missing size")?,
                    )?;
                    total = add(total, 2)?;
                }
                total
            }
            _ => bail!("invalid content kind"),
        };
        active.remove(&id);
        sizes.insert(id, size);
    }
    sizes.get(root).copied().context("missing expansion size")
}

async fn insert_blobs(
    connection: &mut SqliteConnection,
    blobs: &BTreeMap<String, Value>,
) -> Result<()> {
    let rows = blobs
        .iter()
        .map(|(id, body)| Ok((id, serde_json::to_string(body)?)))
        .collect::<Result<Vec<_>>>()?;
    for chunk in rows.chunks(200) {
        let mut query =
            QueryBuilder::<Sqlite>::new("INSERT OR IGNORE INTO zf_content(reference,body) ");
        query.push_values(chunk, |mut row, (id, body)| {
            row.push_bind(*id).push_bind(body);
        });
        query.build().execute(&mut *connection).await?;
    }
    let edges = blobs
        .iter()
        .map(|(id, body)| {
            Ok(dependencies(body)?
                .into_iter()
                .map(|child| (id.clone(), child))
                .collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    for chunk in edges.chunks(200) {
        let mut query =
            QueryBuilder::<Sqlite>::new("INSERT OR IGNORE INTO zf_content_edges(parent,child) ");
        query.push_values(chunk, |mut row, (parent, child)| {
            row.push_bind(parent).push_bind(child);
        });
        query.build().execute(&mut *connection).await?;
    }
    Ok(())
}
impl ContentStore {
    pub async fn new(pool: SqlitePool) -> Result<Self> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS zf_content(reference TEXT PRIMARY KEY, body TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS zf_records(scope TEXT NOT NULL, kind TEXT NOT NULL, key TEXT NOT NULL, value_ref TEXT NOT NULL REFERENCES zf_content(reference), PRIMARY KEY(scope,kind,key))").execute(&pool).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS zf_content_edges(parent TEXT NOT NULL,child TEXT NOT NULL,PRIMARY KEY(parent,child))").execute(&pool).await?;
        Ok(Self::from_pool(pool))
    }
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self {
            pool,
            committed: Arc::new(RwLock::new(HashSet::new())),
        }
    }
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
    pub(crate) fn prepare(&self, value: &Value) -> Result<PreparedContent> {
        let mut pending = BTreeMap::new();
        let reference = encode(value, &mut pending, 0)?;
        let known = self.committed.read().unwrap_or_else(|p| p.into_inner());
        pending.retain(|id, _| !known.contains(id));
        Ok(PreparedContent { reference, pending })
    }
    pub(crate) async fn persist_in(
        &self,
        connection: &mut SqliteConnection,
        prepared: &PreparedContent,
    ) -> Result<()> {
        insert_blobs(connection, &prepared.pending).await
    }
    pub(crate) fn mark_committed(&self, prepared: &PreparedContent) {
        let mut known = self.committed.write().unwrap_or_else(|p| p.into_inner());
        // Bound the cache by count (hashes only, no user content). Eviction affects
        // performance only; SQLite remains the source of truth.
        if known.len() + prepared.pending.len() > 65_536 {
            known.clear();
        }
        known.extend(prepared.pending.keys().take(65_536).cloned());
        known.insert(prepared.reference.clone());
    }
    pub async fn intern(&self, value: &Value) -> Result<String> {
        let prepared = self.prepare(value)?;
        if prepared.pending.is_empty() {
            return Ok(prepared.reference);
        }
        let mut transaction = self.pool.begin().await?;
        self.persist_in(&mut transaction, &prepared).await?;
        transaction.commit().await?;
        self.mark_committed(&prepared);
        Ok(prepared.reference)
    }
    /// Never updates the committed cache: the caller owns this transaction and
    /// may still roll it back. Use prepare/persist_in/mark_committed internally.
    pub async fn intern_in(
        &self,
        connection: &mut SqliteConnection,
        value: &Value,
    ) -> Result<String> {
        let prepared = self.prepare(value)?;
        self.persist_in(connection, &prepared).await?;
        Ok(prepared.reference)
    }
    /// Estimate materialized JSON bytes on the DAG without expanding repeated
    /// children. The result is a conservative upper bound, with overflow rejected.
    pub async fn expanded_size(&self, id: &str) -> Result<u64> {
        let blobs = self
            .export_blobs(&[id.to_owned()])
            .await?
            .into_iter()
            .map(|blob| (blob.reference, blob.body))
            .collect();
        expanded_size_in(id, &blobs)
    }
    pub async fn resolve_with_limit(&self, id: &str, max_bytes: u64) -> Result<Value> {
        let blobs = self
            .export_blobs(&[id.to_owned()])
            .await?
            .into_iter()
            .map(|blob| (blob.reference, blob.body))
            .collect();
        ensure!(
            expanded_size_in(id, &blobs)? <= max_bytes,
            "content expansion exceeds {max_bytes} bytes"
        );
        decode(id, &blobs, 0)
    }
    pub async fn resolve(&self, id: &str) -> Result<Value> {
        let blobs = self
            .export_blobs(&[id.to_owned()])
            .await?
            .into_iter()
            .map(|blob| (blob.reference, blob.body))
            .collect();
        decode(id, &blobs, 0)
    }
    /// The closure is loaded with a recursive SQL query. Hashes are checked on read,
    /// so corruption fails before a hydrated checkpoint can execute a node.
    pub async fn export_blobs(&self, roots: &[String]) -> Result<Vec<ContentBlob>> {
        if roots.is_empty() {
            return Ok(vec![]);
        }
        let mut found = BTreeMap::new();
        for chunk in roots.chunks(300) {
            let mut query = QueryBuilder::<Sqlite>::new("WITH RECURSIVE closure(reference) AS (");
            let mut separated = query.separated(" UNION ");
            for id in chunk {
                separated.push("SELECT ").push_bind_unseparated(id);
            }
            query.push(" UNION SELECT e.child FROM zf_content_edges e JOIN closure c ON e.parent=c.reference) SELECT c.reference,b.body FROM closure c LEFT JOIN zf_content b ON b.reference=c.reference");
            for row in query.build().fetch_all(&self.pool).await? {
                let id: String = row.try_get("reference")?;
                let raw: Option<String> = row.try_get("body")?;
                let body: Value =
                    serde_json::from_str(&raw.context("missing content in closure")?)?;
                ensure!(reference(&body)? == id, "content hash mismatch: {id}");
                dependencies(&body)?;
                found.insert(id, body);
            }
        }
        // An incomplete/corrupt edge index cannot hide a dependency.
        for body in found.values() {
            for child in dependencies(body)? {
                ensure!(found.contains_key(&child), "missing content edge or child");
            }
        }
        Ok(found
            .into_iter()
            .map(|(reference, body)| ContentBlob { reference, body })
            .collect())
    }

    pub async fn import_blobs(&self, blobs: &[ContentBlob]) -> Result<()> {
        let mut pending = BTreeMap::new();
        for blob in blobs {
            ensure!(
                reference(&blob.body)? == blob.reference,
                "content hash mismatch: {}",
                blob.reference
            );
            dependencies(&blob.body)?;
            pending.insert(blob.reference.clone(), blob.body.clone());
        }
        let mut transaction = self.pool.begin().await?;
        insert_blobs(&mut transaction, &pending).await?;
        let missing:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zf_content_edges e LEFT JOIN zf_content b ON b.reference=e.child WHERE b.reference IS NULL)").fetch_one(&mut *transaction).await?;
        ensure!(missing == 0, "incomplete imported content closure");
        transaction.commit().await?;
        Ok(())
    }
    /// Append a single immutable item without materializing or hashing the old
    /// array. BEGIN IMMEDIATE serializes competing appends before reading its head.
    pub async fn append_record_item(
        &self,
        scope: &str,
        kind: &str,
        key: &str,
        item: &Value,
    ) -> Result<String> {
        let mut prepared = self.prepare(item)?;
        let item_ref = prepared.reference.clone();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let previous: Option<String> = sqlx::query_scalar(
            "SELECT value_ref FROM zf_records WHERE scope=? AND kind=? AND key=?",
        )
        .bind(scope)
        .bind(kind)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;
        let parent = if let Some(previous) = previous {
            let raw: String = sqlx::query_scalar("SELECT body FROM zf_content WHERE reference=?")
                .bind(&previous)
                .fetch_one(&mut *tx)
                .await?;
            let body: Value = serde_json::from_str(&raw)?;
            ensure!(
                reference(&body)? == previous
                    && matches!(body["kind"].as_str(), Some("array" | "sequence")),
                "record append target is not a valid array"
            );
            previous
        } else {
            add(json!({"kind":"array"}), &mut prepared.pending)?
        };
        prepared.reference = add(
            json!({"kind":"sequence","parent":parent,"item":item_ref}),
            &mut prepared.pending,
        )?;
        self.persist_in(&mut tx, &prepared).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,?,?,?) ON CONFLICT(scope,kind,key) DO UPDATE SET value_ref=excluded.value_ref").bind(scope).bind(kind).bind(key).bind(&prepared.reference).execute(&mut *tx).await?;
        tx.commit().await?;
        self.mark_committed(&prepared);
        Ok(prepared.reference)
    }
    pub async fn put_record(
        &self,
        scope: &str,
        kind: &str,
        key: &str,
        value: &Value,
    ) -> Result<String> {
        let prepared = self.prepare(value)?;
        let id = prepared.reference.clone();
        let mut tx = self.pool.begin().await?;
        self.persist_in(&mut tx, &prepared).await?;
        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,?,?,?) ON CONFLICT(scope,kind,key) DO UPDATE SET value_ref=excluded.value_ref").bind(scope).bind(kind).bind(key).bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        self.mark_committed(&prepared);
        Ok(id)
    }
    /// Durable compare-and-insert; used to claim an effect before it starts.
    pub async fn claim_record(
        &self,
        scope: &str,
        kind: &str,
        key: &str,
        value: &Value,
    ) -> Result<bool> {
        let prepared = self.prepare(value)?;
        let id = prepared.reference.clone();
        let mut tx = self.pool.begin().await?;
        self.persist_in(&mut tx, &prepared).await?;
        let result = sqlx::query(
            "INSERT OR IGNORE INTO zf_records(scope,kind,key,value_ref) VALUES(?,?,?,?)",
        )
        .bind(scope)
        .bind(kind)
        .bind(key)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.mark_committed(&prepared);
        Ok(result.rows_affected() == 1)
    }
    pub async fn record(&self, scope: &str, kind: &str, key: &str) -> Result<Option<Value>> {
        let id: Option<String> = sqlx::query_scalar(
            "SELECT value_ref FROM zf_records WHERE scope=? AND kind=? AND key=?",
        )
        .bind(scope)
        .bind(kind)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        match id {
            Some(id) => Ok(Some(self.resolve(&id).await?)),
            None => Ok(None),
        }
    }
    pub async fn records(&self, scope: &str) -> Result<Vec<ContentRecord>> {
        sqlx::query(
            "SELECT scope,kind,key,value_ref FROM zf_records WHERE scope=? ORDER BY kind,key",
        )
        .bind(scope)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ContentRecord {
                scope: row.try_get("scope")?,
                kind: row.try_get("kind")?,
                key: row.try_get("key")?,
                value_ref: row.try_get("value_ref")?,
            })
        })
        .collect()
    }
    /// A indexed prefix lookup for one entity kind; callers inspecting a small
    /// catalogue do not need the full session's tool and observation history.
    pub async fn records_of_kind(&self, scope: &str, kind: &str) -> Result<Vec<ContentRecord>> {
        sqlx::query(
            "SELECT scope,kind,key,value_ref FROM zf_records WHERE scope=? AND kind=? ORDER BY key",
        )
        .bind(scope)
        .bind(kind)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(ContentRecord {
                scope: row.try_get("scope")?,
                kind: row.try_get("kind")?,
                key: row.try_get("key")?,
                value_ref: row.try_get("value_ref")?,
            })
        })
        .collect()
    }
}

/// Decode the persisted full-output representation, validating fragment order,
/// stream identity and exact byte length. Shared by runtime and archive readers.
pub fn decode_full_output(value: &Value) -> Result<Vec<u8>> {
    use base64::Engine;
    if let Some(fragments) = value.as_array() {
        let mut bytes = Vec::new();
        for (index, fragment) in fragments.iter().enumerate() {
            ensure!(
                fragment["index"].as_u64() == Some(u64::try_from(index)?),
                "output fragment order mismatch"
            );
            ensure!(
                matches!(fragment["stream"].as_str(), Some("stdout" | "stderr")),
                "invalid output stream"
            );
            bytes.extend(
                base64::engine::general_purpose::STANDARD.decode(
                    fragment["data"]
                        .as_str()
                        .context("invalid output fragment")?,
                )?,
            );
        }
        return Ok(bytes);
    }
    ensure!(value["encoding"] == "base64", "unsupported output encoding");
    let length = value["byteLength"]
        .as_u64()
        .context("invalid output byte length")?;
    let chunks = value["chunks"]
        .as_array()
        .context("invalid output chunks")?;
    let mut bytes = Vec::new();
    for chunk in chunks {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(chunk.as_str().context("invalid output chunk")?)?;
        bytes.extend_from_slice(&decoded);
        ensure!(
            u64::try_from(bytes.len())? <= length,
            "output exceeds recorded length"
        );
    }
    ensure!(
        u64::try_from(bytes.len())? == length,
        "incomplete output content"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn store() -> ContentStore {
        ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn exact_json_and_references_are_not_user_syntax() {
        let store = store().await;
        let value = json!({"null":null,"empty":[],"obj":{},"numbers":[18446744073709551615u64,-9223372036854775808i64,1.0],"text":"🙂\n","reference":"sha256:pretend","history":[{"role":"user","parts":[{"text":"same"}]},{"role":"user","parts":[{"text":"same"}]}]});
        let id = store.intern(&value).await.unwrap();
        assert_eq!(store.resolve(&id).await.unwrap(), value);
        let target = super::tests::store().await;
        target
            .import_blobs(&store.export_blobs(std::slice::from_ref(&id)).await.unwrap())
            .await
            .unwrap();
        assert_eq!(target.resolve(&id).await.unwrap(), value);
    }
    #[tokio::test]
    async fn growing_history_stores_each_prefix_once() {
        let store = store().await;
        let mut history = vec![];
        for i in 0..100 {
            history.push(json!({"role":"user","text":format!("{i}:{}","large ".repeat(100))}));
            store
                .intern(&json!({"history":history,"other":"stable"}))
                .await
                .unwrap();
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zf_content")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!(count < 410, "blob count={count}");
        let bytes: i64 =
            sqlx::query_scalar("SELECT SUM(length(CAST(body AS BLOB))) FROM zf_content")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert!(bytes < 160_000, "bytes={bytes}");
    }
    #[tokio::test]
    async fn append_record_keeps_every_immutable_prefix_without_reencoding_history() {
        let store = store().await;
        let mut fifth = String::new();
        for index in 0..1000 {
            let reference = store
                .append_record_item("run", "fragments", "call", &json!(index))
                .await
                .unwrap();
            if index == 4 {
                fifth = reference;
            }
        }
        assert_eq!(store.resolve(&fifth).await.unwrap(), json!([0, 1, 2, 3, 4]));
        assert_eq!(
            store
                .record("run", "fragments", "call")
                .await
                .unwrap()
                .unwrap(),
            json!((0..1000).collect::<Vec<_>>())
        );
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zf_content")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 2001);
    }
    #[tokio::test]
    async fn untrusted_dag_expansion_is_measured_without_materializing_repeated_children() {
        let store = store().await;
        let mut blobs = BTreeMap::new();
        let mut head = add(json!({"kind":"scalar","value":"leaf"}), &mut blobs).unwrap();
        for _ in 0..35 {
            head = add(
                json!({"kind":"object","entries":{"left":head,"right":head}}),
                &mut blobs,
            )
            .unwrap();
        }
        store
            .import_blobs(
                &blobs
                    .into_iter()
                    .map(|(reference, body)| ContentBlob { reference, body })
                    .collect::<Vec<_>>(),
            )
            .await
            .unwrap();
        assert!(store.expanded_size(&head).await.unwrap() > 256 * 1024 * 1024);
        assert!(
            store
                .resolve_with_limit(&head, 256 * 1024 * 1024)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn growing_unicode_strings_share_chunks_and_legacy_scalars_still_resolve() {
        let store = store().await;
        let text = "é漢🙂\n".repeat(2000);
        let legacy = json!({"kind":"scalar","value":text});
        let legacy_ref = reference(&legacy).unwrap();
        store
            .import_blobs(&[ContentBlob {
                reference: legacy_ref.clone(),
                body: legacy,
            }])
            .await
            .unwrap();
        assert_eq!(store.resolve(&legacy_ref).await.unwrap(), json!(text));
        let id = store.intern(&json!(text)).await.unwrap();
        assert_ne!(id, legacy_ref);
        assert_eq!(store.resolve(&id).await.unwrap(), json!(text));
        assert!(
            store
                .export_blobs(&[id])
                .await
                .unwrap()
                .iter()
                .any(|blob| blob.body["kind"] == "string")
        );
    }
    #[tokio::test]
    async fn rollback_never_populates_committed_cache_and_repeated_content_never_writes() {
        let store = store().await;
        let value = json!({"messages":[{"text":"durability"}]});
        let mut tx = store.pool().begin().await.unwrap();
        let id = store.intern_in(&mut tx, &value).await.unwrap();
        tx.rollback().await.unwrap();
        assert!(store.resolve(&id).await.is_err());
        assert_eq!(store.intern(&value).await.unwrap(), id);
        // A deliberately held write connection proves that an exact cached
        // value requires no further SQL connection/transaction.
        let transaction = store.pool().begin().await.unwrap();
        let repeated =
            tokio::time::timeout(std::time::Duration::from_millis(100), store.intern(&value))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(repeated, id);
        transaction.rollback().await.unwrap();
        assert_eq!(store.resolve(&id).await.unwrap(), value);
    }
    #[tokio::test]
    async fn thousand_message_array_hydrates_without_sequence_stack_recursion() {
        let store = store().await;
        let history = json!(
            (0..1000)
                .map(|n| json!({"text":format!("line {n}"),"same":true}))
                .collect::<Vec<_>>()
        );
        let id = store.intern(&history).await.unwrap();
        assert_eq!(store.resolve(&id).await.unwrap(), history);
        let mut blobs = store.export_blobs(&[id]).await.unwrap();
        blobs.retain(|blob| blob.body["value"] != "line 0");
        let target = super::tests::store().await;
        assert!(target.import_blobs(&blobs).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM zf_content")
            .fetch_one(target.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "failed import must roll back the entire closure");
    }
    #[tokio::test]
    async fn missing_or_corrupt_content_never_hydrates() {
        let store = store().await;
        let id = store.intern(&json!(["a", "b"])).await.unwrap();
        sqlx::query("UPDATE zf_content SET body='{}' WHERE reference=?")
            .bind(&id)
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store.resolve(&id).await.is_err());
        assert!(store.resolve("sha256:missing").await.is_err());
    }
}
