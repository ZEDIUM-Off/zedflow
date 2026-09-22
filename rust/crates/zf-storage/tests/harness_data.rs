use serde_json::{Value, json};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::{path::Path, sync::Arc, time::Duration};
use tempfile::TempDir;
use zf_core::identity::Permission;
use zf_core::identity::Scope;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataError;
use zf_storage::data::DataRegistry;

async fn pool(path: &Path) -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(true)
                .busy_timeout(Duration::from_secs(5)),
        )
        .await
        .unwrap()
}

async fn registry(pool: &SqlitePool, universe: &str) -> DataRegistry {
    let content = ContentStore::new(pool.clone()).await.unwrap();
    DataRegistry::new(pool.clone(), content, universe)
        .await
        .unwrap()
}

async fn fixture() -> (TempDir, SqlitePool, DataRegistry) {
    let root = tempfile::tempdir().unwrap();
    let pool = pool(&root.path().join("data.sqlite")).await;
    let registry = registry(&pool, "runtime-a").await;
    (root, pool, registry)
}

#[tokio::test]
async fn aliases_share_one_entity_and_resident_content_across_all_scopes() {
    let (_root, pool, registry) = fixture().await;
    let flow = Scope::Flow("worker".into());
    let bridge = Scope::Bridge("handoff".into());
    let first = registry
        .create(&flow, "answer", &json!({"text":"A"}))
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM zf_content")
        .fetch_one(&pool)
        .await
        .unwrap();
    registry
        .grant(&flow, "answer", &bridge, "input", Permission::Read)
        .await
        .unwrap();
    registry
        .grant(
            &flow,
            "answer",
            &Scope::Runtime,
            "shared",
            Permission::Write,
        )
        .await
        .unwrap();
    let cloned = registry.clone();
    let from_bridge = cloned.snapshot(&bridge, "input").await.unwrap();
    let from_runtime = registry.snapshot(&Scope::Runtime, "shared").await.unwrap();
    assert_eq!(first.entity_id, from_bridge.entity_id);
    assert_eq!(first.revision, from_runtime.revision);
    assert!(Arc::ptr_eq(&first.value, &from_bridge.value));
    assert!(Arc::ptr_eq(&first.value, &from_runtime.value));
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_content")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    let next = registry
        .publish(
            &Scope::Runtime,
            "shared",
            &first.revision,
            &json!({"text":"B"}),
        )
        .await
        .unwrap();
    let head = registry.snapshot(&flow, "answer").await.unwrap();
    let bridge_head = registry.snapshot(&bridge, "input").await.unwrap();
    assert_eq!(next.revision, head.revision);
    assert!(Arc::ptr_eq(&next.value, &bridge_head.value));
    assert_eq!(*first.value, json!({"text":"A"}));
    assert_eq!(*next.value, json!({"text":"B"}));
}

#[tokio::test]
async fn equal_content_does_not_merge_entity_or_revision_identity() {
    let (_root, pool, registry) = fixture().await;
    let bytes = json!({"same":[1,2,3]});
    let first = registry
        .create(&Scope::Runtime, "one", &bytes)
        .await
        .unwrap();
    let second = registry
        .create(&Scope::Runtime, "two", &bytes)
        .await
        .unwrap();
    assert_ne!(first.entity_id, second.entity_id);
    assert_ne!(first.revision, second.revision);
    assert_eq!(first.content_ref, second.content_ref);
    assert!(Arc::ptr_eq(&first.value, &second.value));
    assert!(matches!(
        registry
            .revision(&Scope::Runtime, "two", &first.revision)
            .await,
        Err(DataError::NotFound)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(DISTINCT content_ref) FROM zf_harness_revisions"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn unique_producer_publication_survives_retries_head_advancement_and_reopening() {
    let (_root, pool, registry) = fixture().await;
    let scope = Scope::Flow("producer".into());
    let first = registry
        .publish_unique(&scope, "answer", None, &json!("A"), "seed")
        .await
        .unwrap();
    let second = registry
        .publish_unique(
            &scope,
            "answer",
            Some(&first.revision),
            &json!("B"),
            "visit-result",
        )
        .await
        .unwrap();
    let latest = registry
        .publish(&scope, "answer", &second.revision, &json!("C"))
        .await
        .unwrap();
    let reopened = DataRegistry::new(
        pool.clone(),
        ContentStore::new(pool.clone()).await.unwrap(),
        registry.universe(),
    )
    .await
    .unwrap();
    let retried = reopened
        .publish_unique(
            &scope,
            "answer",
            Some(&first.revision),
            &json!("B"),
            "visit-result",
        )
        .await
        .unwrap();
    assert_eq!(retried.revision, second.revision);
    assert_eq!(
        reopened.snapshot(&scope, "answer").await.unwrap().revision,
        latest.revision
    );
    assert_eq!(
        reopened
            .publish_unique(&scope, "answer", None, &json!("A"), "seed")
            .await
            .unwrap()
            .entity_id,
        first.entity_id
    );
    for (expected, value) in [
        (Some(&first.revision), json!("different")),
        (Some(&second.revision), json!("B")),
        (None, json!("B")),
    ] {
        assert!(matches!(
            reopened
                .publish_unique(&scope, "answer", expected, &value, "visit-result")
                .await,
            Err(DataError::PublicationMismatch)
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_publications")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}

#[tokio::test]
async fn concurrent_identical_unique_publications_commit_only_one_revision() {
    let (_root, pool, registry) = fixture().await;
    let second = DataRegistry::new(
        pool.clone(),
        ContentStore::new(pool.clone()).await.unwrap(),
        registry.universe(),
    )
    .await
    .unwrap();
    let scope = Scope::Runtime;
    let value = json!({"answer":"ready"});
    let (a, b) = tokio::join!(
        registry.publish_unique(&scope, "dataset", None, &value, "produce-1"),
        second.publish_unique(&scope, "dataset", None, &value, "produce-1")
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.revision, b.revision);
    assert_eq!(a.entity_id, b.entity_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn returning_to_old_bytes_is_a_new_publication_and_old_snapshot_stays_stable() {
    let (_root, pool, registry) = fixture().await;
    let a = registry
        .create(&Scope::Runtime, "state", &json!("A"))
        .await
        .unwrap();
    let b = registry
        .publish(&Scope::Runtime, "state", &a.revision, &json!("B"))
        .await
        .unwrap();
    let again = registry
        .publish(&Scope::Runtime, "state", &b.revision, &json!("A"))
        .await
        .unwrap();
    let unchanged = registry
        .publish(&Scope::Runtime, "state", &again.revision, &json!("A"))
        .await
        .unwrap();
    assert_eq!(a.entity_id, again.entity_id);
    assert_eq!(a.content_ref, again.content_ref);
    assert_ne!(a.revision, again.revision);
    assert_ne!(again.revision, unchanged.revision);
    assert_eq!(again.parent_revision, Some(b.revision.clone()));
    assert_eq!(unchanged.parent_revision, Some(again.revision.clone()));
    assert!(Arc::ptr_eq(&a.value, &again.value));
    assert_eq!(*b.value, json!("B"));
    let historical = registry
        .revision(&Scope::Runtime, "state", &b.revision)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&b.value, &historical.value));
    assert!(matches!(
        registry
            .publish(&Scope::Runtime, "state", &a.revision, &json!("stale"))
            .await,
        Err(DataError::Conflict { .. })
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        4
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_publications_across_independent_pools_have_one_winner() {
    let (root, first_pool, first) = fixture().await;
    let second_pool = pool(&root.path().join("data.sqlite")).await;
    let second = registry(&second_pool, "runtime-a").await;
    let initial = first
        .create(&Scope::Runtime, "state", &json!(0))
        .await
        .unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let left_revision = initial.revision.clone();
    let right_revision = initial.revision.clone();
    let left_barrier = Arc::clone(&barrier);
    let left = tokio::spawn(async move {
        left_barrier.wait().await;
        first
            .publish(&Scope::Runtime, "state", &left_revision, &json!(1))
            .await
    });
    let right = tokio::spawn(async move {
        barrier.wait().await;
        second
            .publish(&Scope::Runtime, "state", &right_revision, &json!(2))
            .await
    });
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let winner = results.iter().find_map(|r| r.as_ref().ok()).unwrap();
    let error = results.iter().find_map(|r| r.as_ref().err()).unwrap();
    assert!(
        matches!(error, DataError::Conflict { expected, actual } if expected == &initial.revision && actual == &winner.revision)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&first_pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        registry(&second_pool, "runtime-a")
            .await
            .snapshot(&Scope::Runtime, "state")
            .await
            .unwrap()
            .revision,
        winner.revision
    );
    first_pool.close().await;
    second_pool.close().await;
}

#[tokio::test]
async fn scopes_and_universes_have_no_implicit_alias_fallback() {
    let (_root, pool, first) = fixture().await;
    let second = registry(&pool, "runtime-b").await;
    let flow = Scope::Flow("same-id".into());
    let bridge = Scope::Bridge("same-id".into());
    let original = first
        .create(&flow, "value", &json!("private"))
        .await
        .unwrap();
    for scope in [&bridge, &Scope::Runtime, &Scope::Flow("another".into())] {
        assert!(matches!(
            first.snapshot(scope, "value").await,
            Err(DataError::NotFound)
        ));
        assert!(matches!(
            first
                .publish(scope, "value", &original.revision, &json!("forged"))
                .await,
            Err(DataError::NotFound)
        ));
    }
    assert!(matches!(
        second.snapshot(&flow, "value").await,
        Err(DataError::NotFound)
    ));
    assert!(matches!(
        second
            .grant(&flow, "value", &Scope::Runtime, "leak", Permission::Read)
            .await,
        Err(DataError::NotFound)
    ));
    let independent = second
        .create(&flow, "value", &json!("other universe"))
        .await
        .unwrap();
    assert_ne!(original.entity_id, independent.entity_id);
    assert!(matches!(
        second.revision(&flow, "value", &original.revision).await,
        Err(DataError::NotFound)
    ));
    assert_eq!(
        *first.snapshot(&flow, "value").await.unwrap().value,
        json!("private")
    );
}

#[tokio::test]
async fn read_aliases_cannot_publish_or_escalate_and_aliases_cannot_be_overwritten() {
    let (_root, pool, registry) = fixture().await;
    let owner = Scope::Flow("owner".into());
    let reader = Scope::Bridge("reader".into());
    let original = registry.create(&owner, "state", &json!(1)).await.unwrap();
    registry
        .grant(&owner, "state", &reader, "state", Permission::Read)
        .await
        .unwrap();
    assert!(matches!(
        registry
            .publish(&reader, "state", &original.revision, &json!(2))
            .await,
        Err(DataError::PermissionDenied)
    ));
    for permission in [Permission::Read, Permission::Write] {
        assert!(matches!(
            registry
                .grant(&reader, "state", &Scope::Runtime, "escalated", permission)
                .await,
            Err(DataError::PermissionDenied)
        ));
    }
    assert!(matches!(
        registry.create(&reader, "state", &json!(3)).await,
        Err(DataError::AliasExists)
    ));
    assert!(matches!(
        registry
            .grant(&owner, "state", &reader, "state", Permission::Write)
            .await,
        Err(DataError::AliasExists)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_entities")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn reopen_keeps_alias_rights_revision_chain_and_rejects_stale_publish() {
    let (root, first_pool, first) = fixture().await;
    let flow = Scope::Flow("flow".into());
    let initial = first.create(&flow, "v", &json!({"v":1})).await.unwrap();
    first
        .grant(&flow, "v", &Scope::Runtime, "v", Permission::Read)
        .await
        .unwrap();
    let latest = first
        .publish(&flow, "v", &initial.revision, &json!({"v":2}))
        .await
        .unwrap();
    drop(first);
    first_pool.close().await;
    let reopened_pool = pool(&root.path().join("data.sqlite")).await;
    let reopened = registry(&reopened_pool, "runtime-a").await;
    let current = reopened.snapshot(&Scope::Runtime, "v").await.unwrap();
    assert_eq!(current.entity_id, initial.entity_id);
    assert_eq!(current.revision, latest.revision);
    assert_eq!(current.parent_revision, Some(initial.revision.clone()));
    assert_eq!(
        *reopened
            .revision(&flow, "v", &initial.revision)
            .await
            .unwrap()
            .value,
        json!({"v":1})
    );
    assert!(matches!(
        reopened
            .publish(&flow, "v", &initial.revision, &json!(3))
            .await,
        Err(DataError::Conflict { .. })
    ));
    assert!(matches!(
        reopened
            .publish(&Scope::Runtime, "v", &latest.revision, &json!(3))
            .await,
        Err(DataError::PermissionDenied)
    ));
    reopened_pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_cold_reads_hydrate_one_shared_arc() {
    let (_root, pool, writer) = fixture().await;
    let bytes = json!({"large":"abcdefgh".repeat(12000),"nested":[1,{"x":true}]});
    writer
        .create(&Scope::Runtime, "source", &bytes)
        .await
        .unwrap();
    writer
        .grant(
            &Scope::Runtime,
            "source",
            &Scope::Flow("reader".into()),
            "alias",
            Permission::Read,
        )
        .await
        .unwrap();
    // A distinct constructor starts with an empty resident cache.
    let reader = registry(&pool, "runtime-a").await;
    let mut tasks = Vec::new();
    for index in 0..24 {
        let reader = reader.clone();
        tasks.push(tokio::spawn(async move {
            if index % 2 == 0 {
                reader.snapshot(&Scope::Runtime, "source").await.unwrap()
            } else {
                reader
                    .snapshot(&Scope::Flow("reader".into()), "alias")
                    .await
                    .unwrap()
            }
        }));
    }
    let snapshots = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(*snapshots[0].value, bytes);
    for snapshot in &snapshots {
        assert!(Arc::ptr_eq(&snapshots[0].value, &snapshot.value));
    }
}

#[tokio::test]
async fn failed_commit_rolls_back_content_and_revision_without_touching_legacy_sessions() {
    let (_root, pool, registry) = fixture().await;
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    let legacy = "{\"status\":\"waiting\",\"untouched\":true}";
    sqlx::query("INSERT INTO runs(id,document) VALUES('legacy',?)")
        .bind(legacy)
        .execute(&pool)
        .await
        .unwrap();
    let first = registry
        .create(&Scope::Runtime, "v", &json!("before"))
        .await
        .unwrap();
    let content_count: i64 = sqlx::query_scalar("SELECT count(*) FROM zf_content")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fixture_reject_publish BEFORE UPDATE ON zf_harness_entities BEGIN SELECT RAISE(ABORT,'fixture rejection'); END").execute(&pool).await.unwrap();
    assert!(matches!(
        registry
            .publish(&Scope::Runtime, "v", &first.revision, &json!("after"))
            .await,
        Err(DataError::Database(_))
    ));
    assert_eq!(
        registry
            .snapshot(&Scope::Runtime, "v")
            .await
            .unwrap()
            .revision,
        first.revision
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_content")
            .fetch_one(&pool)
            .await
            .unwrap(),
        content_count
    );
    sqlx::query("DROP TRIGGER fixture_reject_publish")
        .execute(&pool)
        .await
        .unwrap();
    registry
        .publish(&Scope::Runtime, "v", &first.revision, &json!("after"))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT document FROM runs WHERE id='legacy'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        legacy
    );
    for sql in [
        "UPDATE zf_harness_revisions SET content_ref=content_ref",
        "DELETE FROM zf_harness_revisions",
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err());
    }
}

#[tokio::test]
async fn invalid_names_and_mismatched_content_database_are_rejected() {
    let (_root, pool, registry) = fixture().await;
    for scope in [Scope::Flow("".into()), Scope::Bridge(" \t".into())] {
        assert!(matches!(
            registry.create(&scope, "alias", &Value::Null).await,
            Err(DataError::InvalidName(_))
        ));
    }
    assert!(matches!(
        registry.create(&Scope::Runtime, "a\0b", &Value::Null).await,
        Err(DataError::InvalidName(_))
    ));
    let content = ContentStore::new(pool.clone()).await.unwrap();
    assert!(matches!(
        DataRegistry::new(pool.clone(), content.clone(), "").await,
        Err(DataError::InvalidName(_))
    ));
    let other = tempfile::tempdir().unwrap();
    let other_pool = crate::pool(&other.path().join("other.sqlite")).await;
    assert!(matches!(
        DataRegistry::new(other_pool, content, "runtime").await,
        Err(DataError::PoolMismatch)
    ));
}
