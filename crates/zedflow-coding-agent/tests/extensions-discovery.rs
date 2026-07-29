use zedflow_coding_agent::extensions::discover_and_load_extensions;

#[test]
fn empty_directory_has_no_extensions() {
    let directory =
        std::env::temp_dir().join(format!("zedflow-empty-extensions-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let loaded = discover_and_load_extensions(&directory);
    assert!(loaded.extensions.is_empty());
    let _ = std::fs::remove_dir(&directory);
}
