use zedflow_agent::harness::{
    session::{InMemorySessionStorage, InMemorySessionStorageOptions, Session},
    types::{CustomEntry, LeafEntry, SessionTreeEntry, SessionTreeEntryBase},
};

fn base(id: &str, parent_id: Option<&str>) -> SessionTreeEntryBase {
    SessionTreeEntryBase {
        id: id.into(),
        parent_id: parent_id.map(str::to_owned),
        timestamp: "2025-01-01T00:00:00.000Z".into(),
    }
}

#[tokio::test]
async fn traverses_only_the_path_to_the_active_leaf() {
    let entries = vec![
        SessionTreeEntry::Custom(CustomEntry {
            base: base("root", None),
            custom_type: "data".into(),
            data: None,
        }),
        SessionTreeEntry::Custom(CustomEntry {
            base: base("left", Some("root")),
            custom_type: "data".into(),
            data: None,
        }),
        SessionTreeEntry::Custom(CustomEntry {
            base: base("right", Some("root")),
            custom_type: "data".into(),
            data: None,
        }),
        SessionTreeEntry::Leaf(LeafEntry {
            base: base("leaf", Some("right")),
            target_id: Some("right".into()),
        }),
    ];
    let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
        entries: Some(entries),
        ..Default::default()
    }))
    .unwrap();

    let branch = Session::new(storage).get_branch(None).await;
    assert_eq!(branch.len(), 2);
    assert!(matches!(&branch[0], SessionTreeEntry::Custom(entry) if entry.base.id == "root"));
    assert!(matches!(&branch[1], SessionTreeEntry::Custom(entry) if entry.base.id == "right"));
}
