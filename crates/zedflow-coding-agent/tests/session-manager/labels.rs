use zedflow_agent::harness::session::{InMemorySessionStorage, Session};

#[tokio::test]
async fn labels_follow_the_latest_change_and_can_be_cleared() {
    let session = Session::new(InMemorySessionStorage::default());
    let entry_id = session.append_custom_entry("data", None).await.unwrap();

    session
        .append_label(entry_id.clone(), Some("first".into()))
        .await
        .unwrap();
    session
        .append_label(entry_id.clone(), Some("latest".into()))
        .await
        .unwrap();
    assert_eq!(
        session.get_label(&entry_id).await.as_deref(),
        Some("latest")
    );

    session.append_label(entry_id.clone(), None).await.unwrap();
    assert_eq!(session.get_label(&entry_id).await, None);
}
