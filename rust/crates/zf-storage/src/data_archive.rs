//! Portable registry metadata. Contents stay in the archive's shared CAS closure.
//! Installation joins the transaction which makes the imported run visible.
use crate::content_store::ContentStore;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistryArchive {
    pub entities: Vec<Entity>,
    pub revisions: Vec<Revision>,
    pub aliases: Vec<Alias>,
    pub publications: Vec<Publication>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entity {
    pub id: String,
    pub head: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Revision {
    pub entity: String,
    pub id: String,
    pub parent: Option<String>,
    pub content_ref: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Alias {
    pub scope_kind: String,
    pub scope_id: String,
    pub name: String,
    pub entity: String,
    pub permission: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publication {
    pub id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub alias: String,
    pub entity: String,
    pub revision: String,
    pub expected_revision: Option<String>,
}

fn name(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && !value.contains('\0'),
        "Invalid registry identity"
    );
    Ok(())
}
fn scope(kind: &str, id: &str) -> Result<()> {
    match kind {
        "flow" | "bridge" => name(id),
        "runtime" => {
            ensure!(id.is_empty(), "Runtime scope has no instance ID");
            Ok(())
        }
        _ => anyhow::bail!("Unknown registry scope"),
    }
}
impl RegistryArchive {
    pub fn roots(&self) -> Vec<String> {
        self.revisions
            .iter()
            .map(|r| r.content_ref.clone())
            .collect()
    }
    /// Return parent-before-child order while checking all cross references.
    pub fn validate(&self) -> Result<Vec<&Revision>> {
        let mut entities = BTreeMap::new();
        for entity in &self.entities {
            name(&entity.id)?;
            name(&entity.head)?;
            ensure!(
                entities.insert(&entity.id, &entity.head).is_none(),
                "Duplicate entity"
            );
        }
        let mut revisions = BTreeMap::new();
        for revision in &self.revisions {
            name(&revision.id)?;
            ensure!(
                entities.contains_key(&revision.entity),
                "Revision belongs to absent entity"
            );
            ensure!(
                revisions.insert(&revision.id, revision).is_none(),
                "Duplicate revision identity"
            );
        }
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        while ordered.len() < revisions.len() {
            let before = ordered.len();
            for revision in revisions.values() {
                if seen.contains(&revision.id) {
                    continue;
                }
                if let Some(parent) = &revision.parent {
                    let parent = revisions.get(parent).context("Revision parent is absent")?;
                    ensure!(
                        parent.entity == revision.entity,
                        "Revision parent belongs to another entity"
                    );
                    if !seen.contains(&parent.id) {
                        continue;
                    }
                }
                seen.insert(&revision.id);
                ordered.push(*revision);
            }
            ensure!(ordered.len() > before, "Cyclic registry revisions");
        }
        for (entity, head) in &entities {
            ensure!(
                revisions
                    .get(head)
                    .is_some_and(|revision| revision.entity == **entity),
                "Entity head is absent or foreign"
            );
        }
        let mut aliases = BTreeMap::new();
        for alias in &self.aliases {
            scope(&alias.scope_kind, &alias.scope_id)?;
            name(&alias.name)?;
            ensure!(
                ["read", "write"].contains(&alias.permission.as_str()),
                "Invalid alias permission"
            );
            ensure!(
                entities.contains_key(&alias.entity),
                "Alias entity is absent"
            );
            ensure!(
                aliases
                    .insert((&alias.scope_kind, &alias.scope_id, &alias.name), alias)
                    .is_none(),
                "Duplicate scoped alias"
            );
        }
        let mut publications = BTreeSet::new();
        for publication in &self.publications {
            name(&publication.id)?;
            ensure!(
                publications.insert(&publication.id),
                "Duplicate publication"
            );
            let alias = aliases
                .get(&(
                    &publication.scope_kind,
                    &publication.scope_id,
                    &publication.alias,
                ))
                .context("Publication alias absent")?;
            ensure!(
                alias.entity == publication.entity && alias.permission == "write",
                "Publication is not owned by its writable alias"
            );
            let revision = revisions
                .get(&publication.revision)
                .context("Publication revision absent")?;
            ensure!(
                revision.entity == publication.entity
                    && revision.parent == publication.expected_revision,
                "Publication revision precondition differs"
            );
        }
        Ok(ordered)
    }
    pub async fn validate_contents(&self, store: &ContentStore) -> Result<()> {
        self.validate()?;
        for reference in self.roots() {
            store.expanded_size(&reference).await?;
        }
        Ok(())
    }
    pub async fn install(&self, tx: &mut Transaction<'_, Sqlite>, universe: &str) -> Result<()> {
        let revisions = self.validate()?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM zf_harness_entities WHERE universe_id=?)",
        )
        .bind(universe)
        .fetch_one(&mut **tx)
        .await?;
        ensure!(
            !exists,
            "Registry identity already present; refusing replacement"
        );
        for entity in &self.entities {
            sqlx::query("INSERT INTO zf_harness_entities(universe_id,entity_id,head_revision) VALUES(?,?,?)").bind(universe).bind(&entity.id).bind(&entity.head).execute(&mut **tx).await?;
        }
        for revision in revisions {
            sqlx::query("INSERT INTO zf_harness_revisions(universe_id,entity_id,revision_id,parent_revision,content_ref) VALUES(?,?,?,?,?)").bind(universe).bind(&revision.entity).bind(&revision.id).bind(&revision.parent).bind(&revision.content_ref).execute(&mut **tx).await?;
        }
        for alias in &self.aliases {
            sqlx::query("INSERT INTO zf_harness_aliases(universe_id,scope_kind,scope_id,alias,entity_id,permission) VALUES(?,?,?,?,?,?)").bind(universe).bind(&alias.scope_kind).bind(&alias.scope_id).bind(&alias.name).bind(&alias.entity).bind(&alias.permission).execute(&mut **tx).await?;
        }
        for publication in &self.publications {
            sqlx::query("INSERT INTO zf_harness_publications(universe_id,publication_id,scope_kind,scope_id,alias,entity_id,revision_id,expected_revision) VALUES(?,?,?,?,?,?,?,?)").bind(universe).bind(&publication.id).bind(&publication.scope_kind).bind(&publication.scope_id).bind(&publication.alias).bind(&publication.entity).bind(&publication.revision).bind(&publication.expected_revision).execute(&mut **tx).await?;
        }
        Ok(())
    }
}

pub async fn capture(pool: &SqlitePool, universe: &str) -> Result<RegistryArchive> {
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='zf_harness_entities')").fetch_one(pool).await?;
    if !exists {
        return Ok(RegistryArchive::default());
    }
    let mut tx = pool.begin().await?;
    let mut result = RegistryArchive::default();
    for row in sqlx::query("SELECT entity_id,head_revision FROM zf_harness_entities WHERE universe_id=? ORDER BY entity_id").bind(universe).fetch_all(&mut *tx).await? {result.entities.push(Entity{id:row.try_get(0)?,head:row.try_get(1)?});}
    for row in sqlx::query("SELECT entity_id,revision_id,parent_revision,content_ref FROM zf_harness_revisions WHERE universe_id=? ORDER BY revision_id").bind(universe).fetch_all(&mut *tx).await? {result.revisions.push(Revision{entity:row.try_get(0)?,id:row.try_get(1)?,parent:row.try_get(2)?,content_ref:row.try_get(3)?});}
    for row in sqlx::query("SELECT scope_kind,scope_id,alias,entity_id,permission FROM zf_harness_aliases WHERE universe_id=? ORDER BY scope_kind,scope_id,alias").bind(universe).fetch_all(&mut *tx).await? {result.aliases.push(Alias{scope_kind:row.try_get(0)?,scope_id:row.try_get(1)?,name:row.try_get(2)?,entity:row.try_get(3)?,permission:row.try_get(4)?});}
    for row in sqlx::query("SELECT publication_id,scope_kind,scope_id,alias,entity_id,revision_id,expected_revision FROM zf_harness_publications WHERE universe_id=? ORDER BY publication_id").bind(universe).fetch_all(&mut *tx).await? {result.publications.push(Publication{id:row.try_get(0)?,scope_kind:row.try_get(1)?,scope_id:row.try_get(2)?,alias:row.try_get(3)?,entity:row.try_get(4)?,revision:row.try_get(5)?,expected_revision:row.try_get(6)?});}
    tx.commit().await?;
    result.validate()?;
    Ok(result)
}
