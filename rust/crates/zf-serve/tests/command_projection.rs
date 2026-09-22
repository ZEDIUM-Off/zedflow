use serde_json::json;
use zf_storage::content_store::ContentStore;
use zf_storage::session_store;

#[tokio::test]
async fn command_hydration_never_reads_completed_trace_bodies() {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let store = ContentStore::new(db).await.unwrap();
    let context = json!({"cwd":"/fixture","instructions":[],"skills":[]});
    let mut run = json!({
        "id":"command-run",
        "activities":[{"occurrenceId":"finished","inputRef":"sha256:unavailable-input","outputRef":"sha256:unavailable-output"}],
        "toolActivities":[{"callId":"finished-tool","resultRef":"sha256:unavailable-result"}],
        "queue":[{"id":"pending","status":"pending","text":"Continue"}],
        "messages":[{"id":"message","role":"user","text":"Continue"}]
    });
    for (field, value) in [
        ("state", json!({"answer":"preserved"})),
        ("context", context.clone()),
        ("composition", json!({"id":"flow","nodes":[],"edges":[]})),
        ("flowSource", json!("fn flow() {}")),
        ("input", json!({"input":"Continue"})),
    ] {
        run[format!("{field}Ref")] = json!(store.intern(&value).await.unwrap());
    }
    let command = session_store::hydrate_command(&store, &run).await.unwrap();
    assert_eq!(command["context"], context);
    assert_eq!(command["state"]["answer"], "preserved");
    assert_eq!(command["flowSource"], "fn flow() {}");
    for field in ["activities", "toolActivities", "queue", "messages"] {
        assert_eq!(command[field], run[field]);
    }
    assert!(session_store::hydrate_run(&store, &run).await.is_err());
}
