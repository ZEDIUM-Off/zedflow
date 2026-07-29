//! Pi coding-agent test manifest entry: `tests/resource-loader.rs`.
//!
//! The deterministic Rust contract is owned by the package modules; retain
//! this integration-test target so the frozen package layout stays one-to-one.

#[allow(dead_code)]
pub const TEST_PATH: &str = "tests/resource-loader.rs";

#[test]
fn failed_extension_reload_keeps_active_extensions() {
    use std::fs;
    use zedflow_coding_agent::resource_loader::DefaultResourceLoader;

    let root = std::env::temp_dir().join(format!("zedflow-resource-loader-{}", std::process::id()));
    let extensions = root.join(".pi/extensions");
    fs::create_dir_all(&extensions).unwrap();
    fs::write(extensions.join("active.rs"), "active").unwrap();
    let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
    loader.reload();
    assert_eq!(loader.get_extensions().extensions[0].name, "active");

    fs::remove_dir_all(&extensions).unwrap();
    fs::write(&extensions, "not a directory").unwrap();
    loader.reload();
    assert_eq!(loader.get_extensions().extensions[0].name, "active");
    assert!(!loader.get_extensions().errors.is_empty());
    let _ = fs::remove_dir_all(root);
}
