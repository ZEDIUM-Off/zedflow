#[test]
fn trust_options_offer_persistent_and_session_decisions() {
    let options =
        zedflow_coding_agent::trust_manager::get_project_trust_options(".", true).unwrap();
    assert!(options.iter().any(|option| option.label == "Trust"));
    assert!(
        options
            .iter()
            .any(|option| option.label == "Do not trust (this session only)")
    );
}

#[test]
fn trust_store_persists_and_propagates_invalid_data() {
    use std::fs;
    use zedflow_coding_agent::trust_manager::ProjectTrustStore;

    let root = std::env::temp_dir().join(format!("zedflow-trust-boundary-{}", std::process::id()));
    let project = root.join("project");
    fs::create_dir_all(&project).unwrap();
    let store = ProjectTrustStore::new(&root);
    store.set(&project, Some(true)).unwrap();
    assert_eq!(store.get(&project).unwrap(), Some(true));

    fs::write(root.join("trust.json"), "{invalid").unwrap();
    assert_eq!(
        store.get(&project).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    fs::remove_dir_all(root).unwrap();
}
