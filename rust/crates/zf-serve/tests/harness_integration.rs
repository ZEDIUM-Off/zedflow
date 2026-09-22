use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::ContextBlock;
use zf_context::context::ContextExpr;
use zf_context::context::ContextItem;
use zf_context::context::ContextStrategy;
use zf_context::context::ContextValue;
use zf_context::context::FragmentFormat;
use zf_context::context::FragmentRole;
use zf_context::context::evaluate;
use zf_core::identity::Permission;
use zf_core::identity::Scope;
use zf_core::types::DataType;
use zf_core::types::TypeRegistry;
use zf_storage::content_store::ContentStore;
use zf_storage::data::DataRegistry;

#[tokio::test]
async fn context_passage_pins_revision_and_projects_the_same_arc_after_alias_publication() {
    let root = tempfile::tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(root.path().join("runtime.sqlite"))
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await
        .unwrap();
    let store = ContentStore::new(pool.clone()).await.unwrap();
    let registry = DataRegistry::new(pool, store, "one-runtime").await.unwrap();
    let flow = Scope::Flow("conversation".into());
    let original = registry
        .create(
            &flow,
            "history",
            &json!({"text":"avant","unused":"private"}),
        )
        .await
        .unwrap();
    registry
        .grant(
            &flow,
            "history",
            &Scope::Runtime,
            "shared",
            Permission::Write,
        )
        .await
        .unwrap();
    let captured = registry.snapshot(&Scope::Runtime, "shared").await.unwrap();
    let resources = BTreeMap::from([("history".into(), Arc::clone(&captured.value))]);
    let strategy = ContextStrategy::new("capture", "Passage stable")
        .require(
            "history",
            DataType::Record {
                fields: BTreeMap::from([("text".into(), DataType::Text)]),
            },
        )
        .with_program(vec![
            ContextBlock::emit(
                "text",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::field(ContextExpr::resource("history"), "text"),
            ),
            ContextBlock::emit(
                "projection",
                FragmentRole::Data,
                FragmentFormat::Json,
                ContextExpr::project(ContextExpr::resource("history"), &["text"]),
            ),
        ]);
    let types = TypeRegistry::new();
    let passage = evaluate(&strategy, &resources, &types);
    assert!(passage.complete, "{:?}", passage.diagnostics);
    let published = registry
        .publish(
            &Scope::Runtime,
            "shared",
            &captured.revision,
            &json!({"text":"après","unused":"new"}),
        )
        .await
        .unwrap();
    assert_eq!(published.entity_id, original.entity_id);
    assert_ne!(published.revision, captured.revision);
    assert_eq!(captured.revision, original.revision);

    let ContextItem::Fragment {
        value: ContextValue::Shared { root, pointer },
        ..
    } = &passage.items[0]
    else {
        panic!("Expected a shared field view")
    };
    assert!(Arc::ptr_eq(root, &captured.value));
    assert!(Arc::ptr_eq(root, &original.value));
    assert_eq!(pointer, "/text");
    assert_eq!(root.pointer(pointer).unwrap(), "avant");
    let ContextItem::Fragment {
        value: ContextValue::Object { fields },
        ..
    } = &passage.items[1]
    else {
        panic!("Expected a projection of shared views")
    };
    let ContextValue::Shared {
        root: projected, ..
    } = &fields["text"]
    else {
        panic!("Expected the original Arc")
    };
    assert!(Arc::ptr_eq(projected, root));
    assert_eq!(fields.len(), 1);
    assert_eq!(
        serde_json::to_value(&passage).unwrap()["items"][1]["value"],
        json!({"text":"avant"})
    );
    assert_eq!(
        serde_json::to_value(evaluate(&strategy, &resources, &types)).unwrap(),
        serde_json::to_value(&passage).unwrap()
    );

    let next = registry.snapshot(&flow, "history").await.unwrap();
    let next_passage = evaluate(
        &strategy,
        &BTreeMap::from([("history".into(), Arc::clone(&next.value))]),
        &types,
    );
    assert!(next_passage.complete);
    assert_eq!(
        serde_json::to_value(&next_passage).unwrap()["items"][0]["value"],
        "après"
    );
    let retained = registry
        .revision(&flow, "history", &captured.revision)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&retained.value, &captured.value));
}
