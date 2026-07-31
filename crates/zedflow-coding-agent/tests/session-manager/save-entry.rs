use serde_json::json;
use zedflow_agent::harness::{
    session::{InMemorySessionStorage, Session},
    types::SessionTreeEntry,
};

#[tokio::test]
async fn saved_custom_entry_is_linked_into_the_active_branch() {
    let session = Session::new(InMemorySessionStorage::default());
    let first = session.append_custom_entry("first", None).await.unwrap();
    let second = session
        .append_custom_entry("data", Some(json!({"ok": true})))
        .await
        .unwrap();

    assert!(
        matches!(session.get_entry(&second).await, Some(SessionTreeEntry::Custom(entry)) if entry.base.parent_id.as_deref() == Some(&first) && entry.data == Some(json!({"ok": true})))
    );
    assert_eq!(session.get_branch(None).await.len(), 2);
}
