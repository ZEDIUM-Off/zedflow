use serde_json::json;
use std::collections::BTreeMap;
use zf_context::context::ContextBlock;
use zf_context::context::ContextExpr;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_context::context::evaluate;
use zf_context::window;
use zf_context::window::PreparedWindow;
use zf_context::window::WindowItem as Item;
use zf_context::window::WindowPatch as Patch;
use zf_core::identity::Permission;
use zf_core::identity::Scope;
use zf_core::types::TypeRegistry;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataError;
use zf_storage::data::DataRegistry;
use zf_storage::data::WindowRegistry;
use zf_storage::data::WindowStoreError;
fn fragment(id: &str, text: &str) -> Item {
    Item::Fragment {
        id: id.into(),
        role: FragmentRole::Data,
        format: FragmentFormat::Text,
        value: json!(text),
        sources: vec![],
    }
}
fn prepared() -> PreparedWindow {
    PreparedWindow {
        strategy_id: "strategy".into(),
        strategy_revision: "source-hash".into(),
        program_revision: None,
        items: vec![
            fragment("intro", "A"),
            Item::Group {
                id: "group".into(),
                label: "Details".into(),
                items: vec![fragment("body", "B"), fragment("extra", "C")],
            },
        ],
        source_revisions: BTreeMap::new(),
        capabilities: vec![],
    }
}
async fn fixture() -> (
    tempfile::TempDir,
    sqlx::SqlitePool,
    DataRegistry,
    WindowRegistry,
) {
    let root = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.path().join("windows.sqlite"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let data = DataRegistry::new(pool.clone(), content, "run-a")
        .await
        .unwrap();
    let windows = WindowRegistry::new(data.clone());
    (root, pool, data, windows)
}
#[tokio::test]
async fn patches_publish_one_revision_and_preserve_previous_window_and_captured_contract() {
    let (_root, pool, _, windows) = fixture().await;
    let first = windows
        .create(&Scope::Runtime, "context", &prepared())
        .await
        .unwrap();
    let next = windows
        .patch(
            &Scope::Runtime,
            "context",
            &first.revision,
            &[
                Patch::Move {
                    id: "body".into(),
                    parent: None,
                    index: 0,
                },
                Patch::Replace {
                    id: "intro".into(),
                    item: fragment("intro", "Changed"),
                },
                Patch::Remove { id: "extra".into() },
                Patch::Representation {
                    id: "body".into(),
                    format: FragmentFormat::Json,
                    value: json!({"short":"B"}),
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(next.parent_revision, Some(first.revision.clone()));
    let window = window::decode(next.value.as_ref()).unwrap();
    assert_eq!(window.items[0].id(), "body");
    assert_eq!(window.items[1], fragment("intro", "Changed"));
    assert_eq!(window.strategy_revision, "source-hash");
    assert!(window.capabilities.is_empty());
    let original = windows
        .revision(&Scope::Runtime, "context", &first.revision)
        .await
        .unwrap();
    assert_eq!(window::decode(original.value.as_ref()).unwrap(), prepared());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert!(matches!(
        windows
            .patch(
                &Scope::Runtime,
                "context",
                &first.revision,
                &[Patch::Remove { id: "body".into() }]
            )
            .await,
        Err(WindowStoreError::Data(DataError::Conflict { .. }))
    ));
}
#[tokio::test]
async fn invalid_patch_batch_cannot_commit_a_valid_prefix_or_cycle_the_tree() {
    let (_root, pool, _, windows) = fixture().await;
    let first = windows
        .create(&Scope::Runtime, "context", &prepared())
        .await
        .unwrap();
    for invalid in [
        Patch::Move {
            id: "group".into(),
            parent: Some("body".into()),
            index: 0,
        },
        Patch::Move {
            id: "group".into(),
            parent: Some("group".into()),
            index: 0,
        },
        Patch::Move {
            id: "body".into(),
            parent: None,
            index: 99,
        },
        Patch::Replace {
            id: "body".into(),
            item: fragment("other", "D"),
        },
        Patch::Replace {
            id: "body".into(),
            item: Item::Group {
                id: "body".into(),
                label: "Nested".into(),
                items: vec![fragment("intro", "Duplicate")],
            },
        },
        Patch::Representation {
            id: "body".into(),
            format: FragmentFormat::Text,
            value: json!(123),
        },
    ] {
        let result = windows
            .patch(
                &Scope::Runtime,
                "context",
                &first.revision,
                &[
                    Patch::Replace {
                        id: "intro".into(),
                        item: fragment("intro", "Prefix"),
                    },
                    invalid,
                ],
            )
            .await;
        assert!(matches!(result, Err(WindowStoreError::Invalid(_))));
        assert_eq!(
            windows
                .read(&Scope::Runtime, "context")
                .await
                .unwrap()
                .revision,
            first.revision
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM zf_harness_revisions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn scoped_window_permissions_and_reopening_preserve_durable_revision() {
    let (root, pool, data, windows) = fixture().await;
    let scope = Scope::Flow("agent".into());
    let first = windows
        .create(&scope, "context", &prepared())
        .await
        .unwrap();
    data.grant(
        &scope,
        "context",
        &Scope::Bridge("inspect".into()),
        "shared",
        Permission::Read,
    )
    .await
    .unwrap();
    assert!(matches!(
        windows
            .patch(
                &Scope::Bridge("inspect".into()),
                "shared",
                &first.revision,
                &[Patch::Remove { id: "intro".into() }]
            )
            .await,
        Err(WindowStoreError::Data(DataError::PermissionDenied))
    ));
    let next = windows
        .patch(
            &scope,
            "context",
            &first.revision,
            &[Patch::Remove { id: "intro".into() }],
        )
        .await
        .unwrap();
    drop(windows);
    drop(data);
    pool.close().await;
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.path().join("windows.sqlite"))
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let windows = WindowRegistry::new(DataRegistry::new(pool, content, "run-a").await.unwrap());
    assert_eq!(
        windows
            .read(&Scope::Bridge("inspect".into()), "shared")
            .await
            .unwrap()
            .revision,
        next.revision
    );
    assert_eq!(
        window::decode(
            &windows
                .revision(&scope, "context", &first.revision)
                .await
                .unwrap()
                .value
        )
        .unwrap(),
        prepared()
    );
}
#[test]
fn capture_requires_complete_evaluation_and_exact_source_revisions() {
    let strategy = ContextStrategy::new("capture", "Capture")
        .require("document", zf_core::types::DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "doc",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("document"),
        )]);
    let missing = evaluate(&strategy, &BTreeMap::new(), &TypeRegistry::new());
    assert!(window::capture(&strategy, "v1", &missing, BTreeMap::new()).is_err());
    let resources = BTreeMap::from([("document".into(), std::sync::Arc::new(json!("content")))]);
    let complete = evaluate(&strategy, &resources, &TypeRegistry::new());
    assert!(window::capture(&strategy, "v1", &complete, BTreeMap::new()).is_err());
    let window = window::capture(
        &strategy,
        "v1",
        &complete,
        BTreeMap::from([("document".into(), "entity-rev-1".into())]),
    )
    .unwrap();
    assert_eq!(window.source_revisions["document"], "entity-rev-1");
    assert_eq!(
        window.items[0],
        Item::Fragment {
            id: "doc".into(),
            role: FragmentRole::Data,
            format: FragmentFormat::Text,
            value: json!("content"),
            sources: vec!["document".into()]
        }
    );
}
