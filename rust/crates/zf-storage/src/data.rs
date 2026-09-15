//! Persistent entities with immutable revisions and scoped aliases.
//!
//! The host owns this registry and decides which scopes a caller may use. A scope
//! is an explicit namespace, not an authentication credential. There is no scope
//! fallback: sharing requires a granted alias. Aliases follow the entity head;
//! snapshots keep their selected revision even when that head advances.
use crate::content_store::ContentStore;
use serde_json::Value;
use sqlx::{Row, SqliteConnection, SqlitePool};
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use zf_core::identity::{EntityId, Permission, Revision, Scope};

fn scope_parts(scope: &Scope) -> Result<(&'static str, &str)> {
    match scope {
        Scope::Flow(id) => {
            validate_name(id, "flow scope")?;
            Ok(("flow", id))
        }
        Scope::Bridge(id) => {
            validate_name(id, "bridge scope")?;
            Ok(("bridge", id))
        }
        Scope::Runtime => Ok(("runtime", "")),
    }
}
fn permission_name(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "read",
        Permission::Write => "write",
    }
}

/// A stable revision and its shared resident content. Cloning this snapshot does
/// not clone the JSON. `Arc::make_mut` can only create a caller-local copy.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub entity_id: EntityId,
    pub revision: Revision,
    pub parent_revision: Option<Revision>,
    pub content_ref: String,
    pub value: Arc<Value>,
}

/// Domain errors stay distinguishable from database and content-store failures.
#[derive(Debug)]
pub enum DataError {
    InvalidName(&'static str),
    PoolMismatch,
    NotFound,
    AliasExists,
    PermissionDenied,
    PublicationMismatch,
    Conflict {
        expected: Revision,
        actual: Revision,
    },
    Database(sqlx::Error),
    Content(anyhow::Error),
}
impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(kind) => {
                write!(f, "invalid {kind}: expected a nonempty name without NUL")
            }
            Self::PoolMismatch => {
                f.write_str("registry and content store must share the same SQLite pool")
            }
            Self::NotFound => {
                f.write_str("data alias or revision not found in this scope and universe")
            }
            Self::AliasExists => f.write_str("data alias already exists in this scope"),
            Self::PermissionDenied => f.write_str("data alias does not grant write permission"),
            Self::PublicationMismatch => {
                f.write_str("publication identity was reused with different arguments")
            }
            Self::Conflict { expected, actual } => write!(
                f,
                "data publication conflict: expected {}, current {}",
                expected, actual
            ),
            Self::Database(error) => write!(f, "data registry database error: {error}"),
            Self::Content(error) => write!(f, "data registry content error: {error}"),
        }
    }
}
impl std::error::Error for DataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Content(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
impl From<sqlx::Error> for DataError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}
impl From<anyhow::Error> for DataError {
    fn from(value: anyhow::Error) -> Self {
        Self::Content(value)
    }
}
pub type Result<T> = std::result::Result<T, DataError>;

type ResidentSlot = Arc<AsyncMutex<Weak<Value>>>;

struct ResidentCache {
    slots: HashMap<String, ResidentSlot>,
    insertions_until_collection: usize,
    #[cfg(test)]
    collection_work: usize,
}

impl Default for ResidentCache {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            insertions_until_collection: 1024,
            #[cfg(test)]
            collection_work: 0,
        }
    }
}

impl ResidentCache {
    fn slot(&mut self, content_ref: &str) -> ResidentSlot {
        // Hits neither scan the table nor advance the collection budget.
        if let Some(slot) = self.slots.get(content_ref) {
            return Arc::clone(slot);
        }
        if self.insertions_until_collection == 0 {
            #[cfg(test)]
            {
                self.collection_work += self.slots.capacity();
            }
            // Only dead entries can be removed. A resident Value or another
            // hydration keeps its slot, preserving pointer identity.
            self.slots.retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot
                        .try_lock()
                        .map_or(true, |value| value.strong_count() > 0)
            });
            self.slots.shrink_to(1024);
            // HashMap collection visits capacity, not just occupied entries.
            // Charge its cost to this many new insertions before the next scan.
            self.insertions_until_collection = self.slots.capacity().max(1024);
        }
        self.insertions_until_collection -= 1;
        let slot = Arc::new(AsyncMutex::new(Weak::new()));
        self.slots.insert(content_ref.to_owned(), Arc::clone(&slot));
        slot
    }
}

/// One registry per runtime universe. Clone it to share its resident content
/// cache between flow, bridge and runtime readers. Values are retained weakly;
/// storage, rather than cache residency, owns their lifetime. Independently
/// constructed registries have separate caches, even for the same universe.
#[derive(Clone)]
pub struct DataRegistry {
    pool: SqlitePool,
    content: ContentStore,
    universe: Arc<str>,
    resident: Arc<Mutex<ResidentCache>>,
}

impl DataRegistry {
    /// Initializes only dedicated harness tables, never existing session rows.
    /// `pool` must be a clone of the pool used by `content`, so content and head
    /// publication can commit in the same transaction, including in-memory DBs.
    pub async fn new(
        pool: SqlitePool,
        content: ContentStore,
        universe: impl Into<String>,
    ) -> Result<Self> {
        let universe = universe.into();
        validate_name(&universe, "runtime universe")?;
        if !Arc::ptr_eq(&pool.connect_options(), &content.pool().connect_options()) {
            return Err(DataError::PoolMismatch);
        }
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        for statement in [
            "CREATE TABLE IF NOT EXISTS zf_harness_entities(\
                universe_id TEXT NOT NULL, entity_id TEXT NOT NULL, head_revision TEXT NOT NULL,\
                PRIMARY KEY(universe_id,entity_id),\
                FOREIGN KEY(universe_id,entity_id,head_revision) REFERENCES zf_harness_revisions(universe_id,entity_id,revision_id) DEFERRABLE INITIALLY DEFERRED)",
            "CREATE TABLE IF NOT EXISTS zf_harness_revisions(\
                universe_id TEXT NOT NULL, entity_id TEXT NOT NULL, revision_id TEXT NOT NULL,\
                parent_revision TEXT, content_ref TEXT NOT NULL REFERENCES zf_content(reference),\
                PRIMARY KEY(universe_id,entity_id,revision_id), UNIQUE(universe_id,revision_id),\
                FOREIGN KEY(universe_id,entity_id) REFERENCES zf_harness_entities(universe_id,entity_id),\
                FOREIGN KEY(universe_id,entity_id,parent_revision) REFERENCES zf_harness_revisions(universe_id,entity_id,revision_id))",
            "CREATE TABLE IF NOT EXISTS zf_harness_aliases(\
                universe_id TEXT NOT NULL, scope_kind TEXT NOT NULL CHECK(scope_kind IN ('flow','bridge','runtime')),\
                scope_id TEXT NOT NULL, alias TEXT NOT NULL, entity_id TEXT NOT NULL,\
                permission TEXT NOT NULL CHECK(permission IN ('read','write')),\
                PRIMARY KEY(universe_id,scope_kind,scope_id,alias),\
                FOREIGN KEY(universe_id,entity_id) REFERENCES zf_harness_entities(universe_id,entity_id))",
            "CREATE INDEX IF NOT EXISTS zf_harness_alias_entity ON zf_harness_aliases(universe_id,entity_id)",
            "CREATE INDEX IF NOT EXISTS zf_harness_revision_content ON zf_harness_revisions(content_ref)",
            "CREATE TABLE IF NOT EXISTS zf_harness_publications(\
                universe_id TEXT NOT NULL, publication_id TEXT NOT NULL, scope_kind TEXT NOT NULL, scope_id TEXT NOT NULL, alias TEXT NOT NULL,\
                entity_id TEXT NOT NULL, revision_id TEXT NOT NULL, expected_revision TEXT,\
                PRIMARY KEY(universe_id,publication_id),\
                FOREIGN KEY(universe_id,entity_id,revision_id) REFERENCES zf_harness_revisions(universe_id,entity_id,revision_id))",
            "CREATE TRIGGER IF NOT EXISTS zf_harness_revision_no_update BEFORE UPDATE ON zf_harness_revisions BEGIN SELECT RAISE(ABORT,'harness revisions are immutable'); END",
            "CREATE TRIGGER IF NOT EXISTS zf_harness_revision_no_delete BEFORE DELETE ON zf_harness_revisions BEGIN SELECT RAISE(ABORT,'harness revisions are immutable'); END",
        ] {
            sqlx::query(statement).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(Self {
            pool,
            content,
            universe: universe.into(),
            resident: Arc::new(Mutex::new(ResidentCache::default())),
        })
    }

    pub fn universe(&self) -> &str {
        &self.universe
    }

    /// Creates a fresh entity and its first immutable revision. The initial alias
    /// grants write access. Existing aliases are never silently replaced.
    pub async fn create(&self, scope: &Scope, alias: &str, value: &Value) -> Result<Snapshot> {
        let (kind, scope_id) = scope_parts(scope)?;
        validate_name(alias, "alias")?;
        let prepared = self.content.prepare(value)?;
        let entity_id = EntityId::from(Uuid::new_v4().to_string());
        let revision = Revision::from(Uuid::new_v4().to_string());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        self.require_free_alias(&mut tx, kind, scope_id, alias)
            .await?;
        self.content.persist_in(&mut tx, &prepared).await?;
        sqlx::query(
            "INSERT INTO zf_harness_entities(universe_id,entity_id,head_revision) VALUES(?,?,?)",
        )
        .bind(self.universe())
        .bind(entity_id.as_str())
        .bind(revision.as_str())
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO zf_harness_revisions(universe_id,entity_id,revision_id,content_ref) VALUES(?,?,?,?)")
            .bind(self.universe()).bind(entity_id.as_str()).bind(revision.as_str()).bind(&prepared.reference)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO zf_harness_aliases(universe_id,scope_kind,scope_id,alias,entity_id,permission) VALUES(?,?,?,?,?,'write')")
            .bind(self.universe()).bind(kind).bind(scope_id).bind(alias).bind(entity_id.as_str())
            .execute(&mut *tx).await?;
        tx.commit().await?;
        self.content.mark_committed(&prepared);
        self.hydrate(entity_id, revision, None, prepared.reference)
            .await
    }

    /// Grants another alias to the same entity without copying content or
    /// revisions. The source must grant write access; read-only aliases cannot
    /// grant themselves write access through a different scope.
    pub async fn grant(
        &self,
        source_scope: &Scope,
        source_alias: &str,
        target_scope: &Scope,
        target_alias: &str,
        permission: Permission,
    ) -> Result<()> {
        let (source_kind, source_id) = scope_parts(source_scope)?;
        let (target_kind, target_id) = scope_parts(target_scope)?;
        validate_name(source_alias, "alias")?;
        validate_name(target_alias, "alias")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (entity, _) = self
            .writable_head(&mut tx, source_kind, source_id, source_alias)
            .await?;
        self.require_free_alias(&mut tx, target_kind, target_id, target_alias)
            .await?;
        sqlx::query("INSERT INTO zf_harness_aliases(universe_id,scope_kind,scope_id,alias,entity_id,permission) VALUES(?,?,?,?,?,?)")
            .bind(self.universe()).bind(target_kind).bind(target_id).bind(target_alias)
            .bind(entity.as_str()).bind(permission_name(permission)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Selects the current head and hydrates that exact revision. A concurrent
    /// publication after the SQL read cannot change this snapshot's value.
    pub async fn snapshot(&self, scope: &Scope, alias: &str) -> Result<Snapshot> {
        self.read(scope, alias, None).await
    }

    /// Reads an older revision through an authorized alias of its entity.
    /// A revision belonging to another entity or universe is not accessible.
    pub async fn revision(
        &self,
        scope: &Scope,
        alias: &str,
        revision: &Revision,
    ) -> Result<Snapshot> {
        self.read(scope, alias, Some(revision)).await
    }

    /// Publishes iff the entity head still equals `expected`. A successful
    /// publication always receives a new revision, even when bytes repeat.
    /// Content, revision and head advancement commit atomically.
    pub async fn publish(
        &self,
        scope: &Scope,
        alias: &str,
        expected: &Revision,
        value: &Value,
    ) -> Result<Snapshot> {
        let (kind, scope_id) = scope_parts(scope)?;
        validate_name(alias, "alias")?;
        let prepared = self.content.prepare(value)?;
        let next = Revision::from(Uuid::new_v4().to_string());
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (entity, actual) = self.writable_head(&mut tx, kind, scope_id, alias).await?;
        if actual != *expected {
            return Err(DataError::Conflict {
                expected: expected.clone(),
                actual,
            });
        }
        self.content.persist_in(&mut tx, &prepared).await?;
        sqlx::query("INSERT INTO zf_harness_revisions(universe_id,entity_id,revision_id,parent_revision,content_ref) VALUES(?,?,?,?,?)")
            .bind(self.universe()).bind(entity.as_str()).bind(next.as_str())
            .bind(expected.as_str()).bind(&prepared.reference).execute(&mut *tx).await?;
        let changed = sqlx::query("UPDATE zf_harness_entities SET head_revision=? WHERE universe_id=? AND entity_id=? AND head_revision=?")
            .bind(next.as_str()).bind(self.universe()).bind(entity.as_str()).bind(expected.as_str())
            .execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(DataError::Conflict {
                expected: expected.clone(),
                actual,
            });
        }
        tx.commit().await?;
        self.content.mark_committed(&prepared);
        self.hydrate(entity, next, Some(expected.clone()), prepared.reference)
            .await
    }

    /// Atomically publish and remember a durable producer identity. Retrying an
    /// identical publication returns its original revision even if the head has
    /// since advanced. `None` creates a new entity; `Some` performs a normal CAS.
    pub async fn publish_unique(
        &self,
        scope: &Scope,
        alias: &str,
        expected: Option<&Revision>,
        value: &Value,
        publication_id: &str,
    ) -> Result<Snapshot> {
        let (kind, scope_id) = scope_parts(scope)?;
        validate_name(alias, "alias")?;
        validate_name(publication_id, "publication identity")?;
        let prepared = self.content.prepare(value)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let prior = sqlx::query("SELECT p.scope_kind,p.scope_id,p.alias,p.entity_id,p.revision_id,p.expected_revision,r.content_ref FROM zf_harness_publications p JOIN zf_harness_revisions r ON r.universe_id=p.universe_id AND r.entity_id=p.entity_id AND r.revision_id=p.revision_id WHERE p.universe_id=? AND p.publication_id=?")
            .bind(self.universe()).bind(publication_id).fetch_optional(&mut *tx).await?;
        if let Some(prior) = prior {
            if prior.try_get::<&str, _>("scope_kind")? != kind
                || prior.try_get::<&str, _>("scope_id")? != scope_id
                || prior.try_get::<&str, _>("alias")? != alias
                || prior
                    .try_get::<Option<String>, _>("expected_revision")?
                    .as_deref()
                    != expected.map(Revision::as_str)
                || prior.try_get::<&str, _>("content_ref")? != prepared.reference
            {
                return Err(DataError::PublicationMismatch);
            }
            let entity = EntityId::from(prior.try_get::<String, _>("entity_id")?);
            let (current_entity, _) = self.writable_head(&mut tx, kind, scope_id, alias).await?;
            if current_entity != entity {
                return Err(DataError::PublicationMismatch);
            }
            let revision = Revision::from(prior.try_get::<String, _>("revision_id")?);
            tx.rollback().await?;
            return self
                .hydrate(entity, revision, expected.cloned(), prepared.reference)
                .await;
        }
        let next = Revision::from(Uuid::new_v4().to_string());
        let entity = if let Some(expected) = expected {
            let (entity, actual) = self.writable_head(&mut tx, kind, scope_id, alias).await?;
            if actual != *expected {
                return Err(DataError::Conflict {
                    expected: expected.clone(),
                    actual,
                });
            }
            entity
        } else {
            self.require_free_alias(&mut tx, kind, scope_id, alias)
                .await?;
            EntityId::from(Uuid::new_v4().to_string())
        };
        self.content.persist_in(&mut tx, &prepared).await?;
        if expected.is_none() {
            sqlx::query("INSERT INTO zf_harness_entities(universe_id,entity_id,head_revision) VALUES(?,?,?)")
                .bind(self.universe()).bind(entity.as_str()).bind(next.as_str()).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO zf_harness_aliases(universe_id,scope_kind,scope_id,alias,entity_id,permission) VALUES(?,?,?,?,?,'write')")
                .bind(self.universe()).bind(kind).bind(scope_id).bind(alias).bind(entity.as_str()).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO zf_harness_revisions(universe_id,entity_id,revision_id,parent_revision,content_ref) VALUES(?,?,?,?,?)")
            .bind(self.universe()).bind(entity.as_str()).bind(next.as_str()).bind(expected.map(Revision::as_str)).bind(&prepared.reference).execute(&mut *tx).await?;
        if let Some(expected) = expected {
            let changed = sqlx::query("UPDATE zf_harness_entities SET head_revision=? WHERE universe_id=? AND entity_id=? AND head_revision=?")
                .bind(next.as_str()).bind(self.universe()).bind(entity.as_str()).bind(expected.as_str()).execute(&mut *tx).await?.rows_affected();
            if changed != 1 {
                return Err(DataError::PublicationMismatch);
            }
        }
        sqlx::query("INSERT INTO zf_harness_publications(universe_id,publication_id,scope_kind,scope_id,alias,entity_id,revision_id,expected_revision) VALUES(?,?,?,?,?,?,?,?)")
            .bind(self.universe()).bind(publication_id).bind(kind).bind(scope_id).bind(alias).bind(entity.as_str()).bind(next.as_str()).bind(expected.map(Revision::as_str)).execute(&mut *tx).await?;
        tx.commit().await?;
        self.content.mark_committed(&prepared);
        self.hydrate(entity, next, expected.cloned(), prepared.reference)
            .await
    }

    async fn read(
        &self,
        scope: &Scope,
        alias: &str,
        revision: Option<&Revision>,
    ) -> Result<Snapshot> {
        let (kind, scope_id) = scope_parts(scope)?;
        validate_name(alias, "alias")?;
        let row = sqlx::query("SELECT r.entity_id,r.revision_id,r.parent_revision,r.content_ref FROM zf_harness_aliases a JOIN zf_harness_entities e ON e.universe_id=a.universe_id AND e.entity_id=a.entity_id JOIN zf_harness_revisions r ON r.universe_id=e.universe_id AND r.entity_id=e.entity_id AND r.revision_id=COALESCE(?,e.head_revision) WHERE a.universe_id=? AND a.scope_kind=? AND a.scope_id=? AND a.alias=?")
            .bind(revision.map(Revision::as_str)).bind(self.universe()).bind(kind).bind(scope_id).bind(alias)
            .fetch_optional(&self.pool).await?.ok_or(DataError::NotFound)?;
        self.hydrate(
            EntityId::from(row.try_get::<String, _>("entity_id")?),
            Revision::from(row.try_get::<String, _>("revision_id")?),
            row.try_get::<Option<String>, _>("parent_revision")?
                .map(Revision::from),
            row.try_get("content_ref")?,
        )
        .await
    }

    async fn writable_head(
        &self,
        connection: &mut SqliteConnection,
        kind: &str,
        scope_id: &str,
        alias: &str,
    ) -> Result<(EntityId, Revision)> {
        let row = sqlx::query("SELECT a.entity_id,a.permission,e.head_revision FROM zf_harness_aliases a JOIN zf_harness_entities e ON e.universe_id=a.universe_id AND e.entity_id=a.entity_id WHERE a.universe_id=? AND a.scope_kind=? AND a.scope_id=? AND a.alias=?")
            .bind(self.universe()).bind(kind).bind(scope_id).bind(alias)
            .fetch_optional(connection).await?.ok_or(DataError::NotFound)?;
        if row.try_get::<&str, _>("permission")? != "write" {
            return Err(DataError::PermissionDenied);
        }
        Ok((
            EntityId::from(row.try_get::<String, _>("entity_id")?),
            Revision::from(row.try_get::<String, _>("head_revision")?),
        ))
    }

    async fn require_free_alias(
        &self,
        connection: &mut SqliteConnection,
        kind: &str,
        scope_id: &str,
        alias: &str,
    ) -> Result<()> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM zf_harness_aliases WHERE universe_id=? AND scope_kind=? AND scope_id=? AND alias=?)")
            .bind(self.universe()).bind(kind).bind(scope_id).bind(alias).fetch_one(connection).await?;
        if exists {
            return Err(DataError::AliasExists);
        }
        Ok(())
    }

    async fn hydrate(
        &self,
        entity_id: EntityId,
        revision: Revision,
        parent_revision: Option<Revision>,
        content_ref: String,
    ) -> Result<Snapshot> {
        let slot = {
            let mut cache = self
                .resident
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            cache.slot(&content_ref)
        };
        // This asynchronous lock serializes only hydration of this content hash.
        // No synchronous guard crosses await, and unrelated hashes load in parallel.
        let mut resident = slot.lock().await;
        let value = match resident.upgrade() {
            Some(value) => value,
            None => {
                let value = Arc::new(self.content.resolve(&content_ref).await?);
                *resident = Arc::downgrade(&value);
                value
            }
        };
        Ok(Snapshot {
            entity_id,
            revision,
            parent_revision,
            content_ref,
            value,
        })
    }
}

fn validate_name(value: &str, kind: &'static str) -> Result<()> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(DataError::InvalidName(kind));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_collection_is_amortized_and_hot_hits_do_not_scan() {
        let mut cache = ResidentCache::default();
        let retained: Vec<_> = (0..8192)
            .map(|index| {
                let key = index.to_string();
                let slot = cache.slot(&key);
                (key, slot)
            })
            .collect();
        assert!(cache.collection_work > 0);
        assert!(cache.collection_work <= 4 * retained.len());
        let work = cache.collection_work;
        for _ in 0..4 {
            for (key, slot) in &retained {
                assert!(Arc::ptr_eq(&cache.slot(key), slot));
            }
        }
        assert_eq!(cache.collection_work, work);
    }

    #[test]
    fn collection_preserves_live_values_and_inflight_hydrations_then_reclaims_dead_slots() {
        let mut cache = ResidentCache::default();
        let value = Arc::new(Value::Bool(true));
        let live = cache.slot("live");
        *live.try_lock().unwrap() = Arc::downgrade(&value);
        let live_identity = Arc::downgrade(&live);
        drop(live);
        let inflight = cache.slot("inflight");
        drop(cache.slot("dead"));
        for index in 0..1024 {
            drop(cache.slot(&format!("first-{index}")));
        }
        assert!(Arc::ptr_eq(
            &cache.slot("live"),
            &live_identity.upgrade().unwrap()
        ));
        assert!(Arc::ptr_eq(&cache.slot("inflight"), &inflight));
        assert!(!cache.slots.contains_key("dead"));
        drop(value);
        drop(inflight);
        for index in 0..=cache.insertions_until_collection {
            drop(cache.slot(&format!("second-{index}")));
        }
        assert!(!cache.slots.contains_key("live"));
        assert!(!cache.slots.contains_key("inflight"));
        assert!(live_identity.upgrade().is_none());
    }
}
