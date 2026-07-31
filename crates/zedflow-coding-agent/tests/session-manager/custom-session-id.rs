use zedflow_agent::harness::{
    session::{InMemorySessionStorage, InMemorySessionStorageOptions, Session},
    types::SessionMetadata,
};

#[tokio::test]
async fn in_memory_session_retains_the_supplied_session_id() {
    let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
        metadata: Some(SessionMetadata {
            id: "custom-session-id".into(),
            created_at: "2025-01-01T00:00:00.000Z".into(),
        }),
        ..Default::default()
    }))
    .unwrap();

    assert_eq!(
        Session::new(storage).get_metadata().await.id,
        "custom-session-id"
    );
}
