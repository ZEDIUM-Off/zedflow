//! Replay boundaries and canonical previews, independent of transport choice.
use serde_json::{Value, json};
use zf_storage::content_store::ContentStore;
use zf_storage::session_store;
use zf_storage::session_sync::SessionSync;
use zf_storage::session_sync::timeline_page;

#[tokio::test]
async fn idle_heartbeat_never_reads_storage_and_tool_delta_has_no_conversation() {
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = ContentStore::new(db.clone()).await.unwrap();
    let sync = SessionSync::new(
        db.clone(),
        store,
        std::sync::Arc::new(zf_runtime::archive_validation::RuntimeArchiveValidation),
    );
    let timeline: Vec<Value> = (0..1000)
        .map(|i| json!({"id":format!("m{i}"),"seq":i,"text":"context ".repeat(100)}))
        .collect();
    let before = json!({"id":"run","workspaceId":"w","timeline":timeline,"toolActivities":[{"callId":"tool","status":"running","outputRef":"sha256:old"}]});
    let mut after = before.clone();
    after["toolActivities"][0]["outputRef"] = json!("sha256:new");
    let changes = zf_storage::session_sync::changes(&before, &after);
    assert_eq!(changes.as_array().unwrap().len(), 1);
    assert_eq!(changes[0]["collection"], "toolActivities");
    assert!(changes.to_string().len() < 300);
    sync.committed("run", after, 1000).await;
    sync.committed("new", json!({"id":"new","workspaceId":"w"}), 0)
        .await;
    db.close().await;
    assert_eq!(
        sync.payload("new", Some(0)).await.unwrap()["type"],
        "heartbeat"
    );
    for _ in 0..3 {
        let heartbeat = sync.payload("run", Some(1000)).await.unwrap();
        assert_eq!(heartbeat["type"], "heartbeat");
        assert!(heartbeat.get("run").is_none());
        assert!(heartbeat.to_string().len() < 150);
    }
}

#[test]
fn pagination_preserves_all_siblings_of_a_sequence() {
    let timeline: Vec<Value> = (0..250)
        .map(|i| json!({"id":format!("m{i}"),"seq":i/7}))
        .collect();
    let (recent, more, cursor) = timeline_page(&timeline);
    assert!(more);
    assert!(recent.len() >= 100);
    let earlier: Vec<_> = timeline
        .iter()
        .filter(|e| e["seq"].as_i64().unwrap() < cursor.unwrap())
        .cloned()
        .collect();
    let mut reconstructed = earlier;
    reconstructed.extend(recent);
    assert_eq!(reconstructed, timeline);
    assert_eq!(timeline_page(&[]), (vec![], false, None));
}

#[tokio::test]
async fn incremental_tool_output_survives_compaction_without_replaying_the_prefix() {
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = ContentStore::new(db).await.unwrap();
    let tool = json!({"callId":"call","arguments":{"path":"example"},"output":"first ","status":"running"});
    let run = json!({"toolActivities":[tool],"timeline":[{"id":"tool:call","seq":1,"updatedSeq":1,"kind":"tool","activity":tool}]});
    let mut compact = session_store::compact_run(&store, &run).await.unwrap();
    let prefix = compact["timeline"][0]["activity"]["outputRef"].clone();
    let event = json!({"type":"tool_progress","callId":"call","chunk":"second"});
    session_store::restore_progress(&store, &mut compact, &event)
        .await
        .unwrap();
    zf_storage::timeline::reconcile(&mut compact, &event, 2);
    let compact = session_store::compact_run(&store, &compact).await.unwrap();
    let restored = session_store::hydrate_tool(&store, &compact["timeline"][0]["activity"])
        .await
        .unwrap();
    assert_eq!(restored["output"], "first second");
    assert_ne!(compact["timeline"][0]["activity"]["outputRef"], prefix);
}

#[tokio::test]
async fn invocation_indices_never_repeat_the_frozen_skill_catalogue() {
    let db = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let store = ContentStore::new(db.clone()).await.unwrap();
    let sync = SessionSync::new(
        db,
        store.clone(),
        std::sync::Arc::new(zf_runtime::archive_validation::RuntimeArchiveValidation),
    );
    let catalogue: Vec<Value> = (0..222)
        .map(|i| json!({"name":format!("skill-{i}"),"description":"long description ".repeat(50)}))
        .collect();
    let snapshots: Vec<Value> = (0..37)
        .map(|i| json!({"invocationId":format!("call-{i}"),"agentPath":"agent","skillCatalog":catalogue,"loaded":{"instructions":"exact instructions"}}))
        .collect();
    let run = json!({"id":"run","contextSnapshots":snapshots});
    let compact = session_store::compact_run(&store, &run).await.unwrap();
    assert!(compact["contextSnapshots"].to_string().len() < 10_000);
    let restored = session_store::hydrate_run(&store, &compact).await.unwrap();
    for (index, original) in snapshots.iter().enumerate() {
        let mut expected = original.clone();
        expected["contentRef"] = compact["contextSnapshots"][index]["contentRef"].clone();
        assert!(
            restored["contextSnapshots"][index] == expected,
            "capture {index} changed"
        );
    }
    assert_eq!(
        session_store::compact_run(&store, &restored).await.unwrap(),
        compact
    );

    // Existing v2 projections and replay documents may still carry catalogues.
    // They remain fully reconstructible, but bootstrap and deltas send indices.
    let mut legacy = compact.clone();
    for snapshot in legacy["contextSnapshots"].as_array_mut().unwrap() {
        snapshot["skillCatalog"] = json!(catalogue);
    }
    let wire = sync.wire_run(legacy.clone()).await.unwrap();
    assert!(wire.to_string().len() < 10_000);
    let deltas = zf_storage::session_sync::changes(&json!({}), &legacy);
    assert!(deltas.to_string().len() < 15_000);
    assert_eq!(
        store
            .resolve(
                compact["contextSnapshots"][0]["contentRef"]
                    .as_str()
                    .unwrap()
            )
            .await
            .unwrap(),
        snapshots[0]
    );
}
