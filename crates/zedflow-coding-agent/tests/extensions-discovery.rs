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

#[test]
fn typescript_and_javascript_extensions_are_deferred_with_diagnostics() {
    let directory = std::env::temp_dir().join(format!(
        "zedflow-deferred-extensions-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let extensions = directory.join(".pi/extensions");
    std::fs::create_dir_all(&extensions).unwrap();
    std::fs::write(extensions.join("extension.ts"), "export default () => {};").unwrap();
    std::fs::write(extensions.join("extension.js"), "export default () => {};").unwrap();
    std::fs::write(extensions.join("README.md"), "not an extension").unwrap();

    let loaded = discover_and_load_extensions(&directory);

    assert!(loaded.extensions.is_empty());
    assert_eq!(loaded.errors.len(), 2);
    assert!(
        loaded
            .errors
            .iter()
            .all(|error| error.message.contains("deferred TypeScript/jiti extension"))
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn package_index_is_discovered_but_inert_files_are_ignored() {
    let directory = std::env::temp_dir().join(format!(
        "zedflow-package-extensions-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package = directory.join(".pi/extensions/example");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("index.ts"), "export default () => {};").unwrap();
    std::fs::write(package.join("README.md"), "not an extension").unwrap();

    let loaded = discover_and_load_extensions(&directory);

    assert!(loaded.extensions.is_empty());
    assert_eq!(loaded.errors.len(), 1);
    assert!(
        loaded.errors[0]
            .message
            .contains("deferred TypeScript/jiti extension")
    );
    let _ = std::fs::remove_dir_all(directory);
}
