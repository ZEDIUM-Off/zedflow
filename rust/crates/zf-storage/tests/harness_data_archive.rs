use serde_json::json;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::Arc;
use zf_core::identity::Permission;
use zf_core::identity::Scope;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataRegistry;
use zf_storage::data_archive;

async fn registry() -> (SqlitePool, ContentStore, DataRegistry) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let registry = DataRegistry::new(pool.clone(), content.clone(), "run")
        .await
        .unwrap();
    (pool, content, registry)
}

#[tokio::test]
async fn portable_registry_retains_identity_shared_revisions_permissions_and_publication_replay() {
    let (source_pool, source, reg) = registry().await;
    let flow = Scope::Flow("worker".into());
    let bridge = Scope::Bridge("review".into());
    let original = reg
        .create(&flow, "document", &json!({"text":"original"}))
        .await
        .unwrap();
    reg.grant(&flow, "document", &bridge, "selected", Permission::Write)
        .await
        .unwrap();
    let final_value = json!({"text":"changed"});
    let latest = reg
        .publish_unique(
            &bridge,
            "selected",
            Some(&original.revision),
            &final_value,
            "edit-1",
        )
        .await
        .unwrap();
    let archive = data_archive::capture(&source_pool, "run").await.unwrap();
    assert_eq!(archive.entities.len(), 1);
    assert_eq!(archive.revisions.len(), 2);
    let (target_pool, target, target_reg) = registry().await;
    target
        .import_blobs(&source.export_blobs(&archive.roots()).await.unwrap())
        .await
        .unwrap();
    archive.validate_contents(&target).await.unwrap();
    // A rolled-back installation never publishes half a registry.
    let mut tx = target_pool.begin().await.unwrap();
    archive.install(&mut tx, "run").await.unwrap();
    tx.rollback().await.unwrap();
    assert!(
        data_archive::capture(&target_pool, "run")
            .await
            .unwrap()
            .entities
            .is_empty()
    );
    let mut tx = target_pool.begin().await.unwrap();
    archive.install(&mut tx, "run").await.unwrap();
    tx.commit().await.unwrap();
    let a = target_reg.snapshot(&flow, "document").await.unwrap();
    let b = target_reg.snapshot(&bridge, "selected").await.unwrap();
    assert_eq!(a.entity_id, original.entity_id);
    assert_eq!(a.revision, latest.revision);
    assert!(Arc::ptr_eq(&a.value, &b.value));
    let replay = target_reg
        .publish_unique(
            &bridge,
            "selected",
            Some(&original.revision),
            &final_value,
            "edit-1",
        )
        .await
        .unwrap();
    assert_eq!(replay.revision, latest.revision);
    assert_eq!(
        data_archive::capture(&target_pool, "run")
            .await
            .unwrap()
            .revisions
            .len(),
        2
    );
    let mut tx = target_pool.begin().await.unwrap();
    assert!(archive.install(&mut tx, "run").await.is_err());
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn missing_contents_and_foreign_revision_parents_are_rejected_before_installation() {
    let (pool, content, reg) = registry().await;
    let flow = Scope::Flow("source".into());
    reg.create(&flow, "one", &json!(1)).await.unwrap();
    reg.create(&flow, "two", &json!(2)).await.unwrap();
    let mut archive = data_archive::capture(&pool, "run").await.unwrap();
    archive.revisions[0].parent = Some(archive.revisions[1].id.clone());
    assert!(archive.validate().is_err());
    archive.revisions[0].parent = None;
    archive.revisions[0].content_ref = "absent".into();
    assert!(archive.validate_contents(&content).await.is_err());
}
