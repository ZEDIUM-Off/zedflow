use adk_graph::{ExecutionConfig, State, checkpoint::Checkpointer};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};
use zf_flows::schema::Composition;
use zf_runtime::runtime::RunServices;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::content_store::ContentStore;

fn node(id: &str, kind: &str, config: Value) -> Value {
    json!({"id":id,"position":{"x":0,"y":0},"data":{"label":id,"kind":kind,"config":config}})
}
fn flow(nodes: Vec<Value>, edges: &[(&str, &str)]) -> Composition {
    serde_json::from_value(json!({"id":"cas-fixture","name":"CAS fixture","nodes":nodes,"edges":edges.iter().enumerate().map(|(i,(source,target))|json!({"id":i.to_string(),"source":source,"target":target})).collect::<Vec<_>>()})).unwrap()
}
async fn resources(root: &Path) -> (ContentStore, Arc<StoredCheckpointer>, Arc<RunServices>) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.join("store.db"))
                .create_if_missing(true)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
        )
        .await
        .unwrap();
    let store = ContentStore::new(pool).await.unwrap();
    let cp = Arc::new(StoredCheckpointer::new(
        zf_storage::contracts::CheckpointStore::new(store.clone())
            .await
            .unwrap(),
    ));
    let services = RunServices::new(
        "nested-cas".into(),
        root.into(),
        root.join("data"),
        ContextSnapshot {
            cwd: root.into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_content_store(store.clone());
    (store, cp, services)
}
async fn roundtrip(version: u32) {
    let root = tempfile::tempdir().unwrap();
    let mut child = flow(
        vec![
            node("s", "start", json!({})),
            node(
                "edit",
                "set",
                json!({"field":"input","value":"{{input}}-child"}),
            ),
            node(
                "effect",
                "tool",
                json!({"tool":"exec","arguments":{"command":"printf x >> visits"}}),
            ),
            node("model", "agent", json!({"modelBinding":"runtime"})),
            node("out", "output", json!({"text":"{{input}}"})),
            node("e", "end", json!({})),
        ],
        &[
            ("s", "edit"),
            ("edit", "effect"),
            ("effect", "model"),
            ("model", "out"),
            ("out", "e"),
        ],
    );
    child.format_version = version;
    let mut doc = flow(
        vec![
            node("s", "start", json!({})),
            node("child", "subgraph", json!({"composition":child})),
            node("out", "output", json!({"text":"{{output}}"})),
            node("e", "end", json!({})),
        ],
        &[("s", "child"), ("child", "out"), ("out", "e")],
    );
    doc.format_version = version;
    let (store, cp, services) = resources(root.path()).await;
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(cp.clone()))
            .unwrap();
    let error = graph
        .invoke(
            State::from([("input".into(), json!("original"))]),
            ExecutionConfig::new("nested-cas"),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, adk_graph::GraphError::Interrupted(_)));
    let saved = cp.load("nested-cas").await.unwrap().unwrap();
    let headers = cp.storage().list_run_headers("nested-cas").await.unwrap();
    let records = store.records("nested-cas").await.unwrap();
    let mut roots: Vec<_> = headers.iter().map(|h| h.checkpoint_ref.clone()).collect();
    roots.extend(records.iter().map(|r| r.value_ref.clone()));
    let blobs = store.export_blobs(&roots).await.unwrap();
    let target = ContentStore::new(
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap(),
    )
    .await
    .unwrap();
    target.import_blobs(&blobs).await.unwrap();
    let imported = Arc::new(StoredCheckpointer::new(
        zf_storage::contracts::CheckpointStore::new(target.clone())
            .await
            .unwrap(),
    ));
    imported.storage().install_headers(&headers).await.unwrap();
    for record in records {
        target
            .put_record(
                &record.scope,
                &record.kind,
                &record.key,
                &store.resolve(&record.value_ref).await.unwrap(),
            )
            .await
            .unwrap();
    }
    assert_eq!(
        serde_json::to_value(imported.load("nested-cas").await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(&saved).unwrap()
    );
    drop(graph);
    drop(cp);
    let services = RunServices::new(
        "nested-cas".into(),
        root.path().into(),
        root.path().join("data"),
        ContextSnapshot {
            cwd: root.path().into(),
            ..Default::default()
        },
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_content_store(target);
    services.set_binding("child/model".into(), json!({"provider":"fixture"}));
    let graph =
        zf_runtime::materialize::build_with_services(&doc, services, None, Some(imported)).unwrap();
    let state = graph
        .invoke(
            State::new(),
            ExecutionConfig::new("nested-cas").with_resume_from(&saved.checkpoint_id),
        )
        .await
        .unwrap();
    assert_eq!(state["response"], "original-child");
    assert_eq!(
        std::fs::read_to_string(root.path().join("visits")).unwrap(),
        "x"
    );
}

#[tokio::test]
async fn v1_nested_cas_export_import_resumes_without_repeating_effects() {
    roundtrip(1).await;
}
#[tokio::test]
async fn v2_nested_cas_export_import_resumes_without_repeating_effects() {
    roundtrip(2).await;
}
