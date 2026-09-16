#![cfg(unix)]
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use zf_storage::flow_packages::{capture, capture_with_preconditions};

fn package(root: &Path, id: &str, files: &[(&str, &[u8])], dependencies: &[(&str, &str)]) {
    fs::create_dir_all(root).unwrap();
    let dependencies: BTreeMap<_, _> = dependencies
        .iter()
        .map(|(alias, path)| (*alias, json!({"path":path})))
        .collect();
    let manifest = json!({"formatVersion":1,"id":id,"name":"Same human name", "entry":"flow.rs",
        "files":files.iter().map(|(name, _)| *name).collect::<Vec<_>>(), "dependencies":dependencies });
    fs::write(
        root.join("flow.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();
    for (name, bytes) in files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}
fn basic(root: &Path, id: &str, deps: &[(&str, &str)]) {
    package(root, id, &[("flow.rs", b"// fixture\n")], deps);
}

#[tokio::test]
async fn captures_exact_binary_inventory_and_all_closure_preconditions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let dependency = temp.path().join("dependency");
    package(
        &root,
        "root",
        &[
            ("flow.rs", b"// root\n"),
            ("assets/binary.dat", &[0, 255, 1]),
            ("module.rs", b"// module"),
        ],
        &[("helper", "../dependency")],
    );
    basic(&dependency, "dependency", &[]);
    let captured = capture_with_preconditions(&root).await.unwrap();
    assert_eq!(captured.snapshot.packages.len(), 2);
    let node = captured.snapshot.root_node().unwrap();
    assert_eq!(
        node.manifest_source.as_bytes(),
        fs::read(root.join("flow.json")).unwrap()
    );
    assert_eq!(node.files["assets/binary.dat"], [0, 255, 1]);
    assert_eq!(captured.preconditions.len(), 6);
    for condition in captured.preconditions {
        assert!(condition.path.is_absolute());
        assert_eq!(
            condition.hash,
            format!("{:x}", Sha256::digest(fs::read(condition.path).unwrap()))
        );
    }
    assert_eq!(captured.snapshot, capture(&root).await.unwrap());
}

#[tokio::test]
async fn secondary_and_dependency_edits_change_the_correct_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let dep = temp.path().join("dep");
    package(
        &root,
        "root",
        &[("flow.rs", b"root"), ("secondary.rs", b"old")],
        &[("dep", "../dep")],
    );
    basic(&dep, "dep", &[]);
    let first = capture(&root).await.unwrap();
    fs::write(root.join("secondary.rs"), b"new").unwrap();
    let second = capture(&root).await.unwrap();
    assert_ne!(first.root, second.root);
    assert_ne!(
        first.root_node().unwrap().content_revision(),
        second.root_node().unwrap().content_revision()
    );
    assert_eq!(
        first.root_node().unwrap().dependencies_revision(),
        second.root_node().unwrap().dependencies_revision()
    );
    fs::write(dep.join("flow.rs"), b"new dependency").unwrap();
    let third = capture(&root).await.unwrap();
    assert_ne!(second.root, third.root);
    assert_eq!(
        second.root_node().unwrap().content_revision(),
        third.root_node().unwrap().content_revision()
    );
    assert_ne!(
        second.root_node().unwrap().dependencies_revision(),
        third.root_node().unwrap().dependencies_revision()
    );
}

#[tokio::test]
async fn shares_diamond_dependencies_and_allows_explicit_nested_packages() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    basic(&root, "root", &[("a", "deps/a"), ("b", "deps/b")]);
    basic(&root.join("deps/a"), "a", &[("shared", "../../shared")]);
    basic(&root.join("deps/b"), "b", &[("shared", "../../shared")]);
    basic(&root.join("shared"), "shared", &[]);
    let captured = capture_with_preconditions(&root).await.unwrap();
    assert_eq!(captured.snapshot.packages.len(), 4);
    assert_eq!(captured.preconditions.len(), 8);
    fs::write(root.join("deps/stray.txt"), "not declared").unwrap();
    assert!(
        capture(&root)
            .await
            .unwrap_err()
            .to_string()
            .contains("undeclared")
    );
}

#[tokio::test]
async fn duplicate_alias_targets_share_one_node_but_conflicting_ids_fail() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    basic(&root, "root", &[("one", "../dep"), ("two", "../dep")]);
    basic(&temp.path().join("dep"), "dep", &[]);
    assert_eq!(capture(&root).await.unwrap().packages.len(), 2);
    basic(&root, "root", &[("one", "../dep"), ("two", "../other")]);
    package(
        &temp.path().join("other"),
        "dep",
        &[("flow.rs", b"conflicting")],
        &[],
    );
    assert!(
        capture(&root)
            .await
            .unwrap_err()
            .to_string()
            .contains("conflicting revisions")
    );
}

#[tokio::test]
async fn rejects_dependency_cycles_and_missing_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    basic(&root, "root", &[("dep", "../dep")]);
    assert!(capture(&root).await.is_err());
    basic(&temp.path().join("dep"), "dep", &[("root", "../root")]);
    assert!(
        capture(&root)
            .await
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
}

#[tokio::test]
async fn rejects_undeclared_files_empty_directories_and_missing_inventory() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    basic(root, "root", &[]);
    fs::write(root.join(".env"), "fixture-secret").unwrap();
    assert!(
        capture(root)
            .await
            .unwrap_err()
            .to_string()
            .contains("undeclared")
    );
    fs::remove_file(root.join(".env")).unwrap();
    fs::create_dir(root.join("target")).unwrap();
    assert!(
        capture(root)
            .await
            .unwrap_err()
            .to_string()
            .contains("undeclared")
    );
    fs::remove_dir(root.join("target")).unwrap();
    fs::remove_file(root.join("flow.rs")).unwrap();
    assert!(
        capture(root)
            .await
            .unwrap_err()
            .to_string()
            .contains("inventory")
    );
}

#[tokio::test]
async fn rejects_symlinked_root_components_dependencies_and_inventory() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let dep = temp.path().join("dep");
    basic(&root, "root", &[]);
    basic(&dep, "dep", &[]);
    symlink(&root, temp.path().join("link")).unwrap();
    assert!(capture(&temp.path().join("link")).await.is_err());
    // A lexical normalizer must not erase a symlink preceding a parent component.
    assert!(capture(&temp.path().join("link/../root")).await.is_err());
    symlink(&dep, root.join("linked-dependency")).unwrap();
    basic(&root, "root", &[("dep", "linked-dependency")]);
    assert!(capture(&root).await.is_err());
    fs::remove_file(root.join("linked-dependency")).unwrap();
    basic(&root, "root", &[]);
    fs::remove_file(root.join("flow.rs")).unwrap();
    let secret = temp.path().join("secret");
    fs::write(&secret, "DO-NOT-READ-SECRET").unwrap();
    symlink(&secret, root.join("flow.rs")).unwrap();
    let error = format!("{:#}", capture(&root).await.unwrap_err());
    assert!(!error.contains("DO-NOT-READ-SECRET"));
    assert!(error.contains("symlink") || error.contains("regular"));
    fs::remove_file(root.join("flow.rs")).unwrap();
    fs::write(root.join("flow.rs"), "root").unwrap();
    symlink(&dep, root.join("assets")).unwrap();
    assert!(capture(&root).await.is_err());
}

#[tokio::test]
async fn refuses_special_files_without_blocking_or_reading_them() {
    use nix::{sys::stat::Mode, unistd::mkfifo};
    let temp = tempfile::tempdir().unwrap();
    basic(temp.path(), "root", &[]);
    fs::remove_file(temp.path().join("flow.rs")).unwrap();
    mkfifo(&temp.path().join("flow.rs"), Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
    let error = tokio::time::timeout(std::time::Duration::from_secs(2), capture(temp.path()))
        .await
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("special"));
}

#[tokio::test]
async fn rejects_file_traversal_absolute_paths_and_oversized_sparse_files() {
    let temp = tempfile::tempdir().unwrap();
    basic(temp.path(), "root", &[]);
    let path = temp.path().join("flow.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for invalid in ["../secret", "/etc/passwd", "nested/../../secret"] {
        manifest["files"] = json!(["flow.rs", invalid]);
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(capture(temp.path()).await.is_err());
    }
    basic(temp.path(), "root", &[]);
    fs::OpenOptions::new()
        .write(true)
        .open(temp.path().join("flow.rs"))
        .unwrap()
        .set_len(zf_flows::package::MAX_PACKAGE_FILE_BYTES as u64 + 1)
        .unwrap();
    assert!(
        capture(temp.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("byte limit")
    );
}

#[tokio::test]
async fn rejects_oversized_manifests_and_excessive_directory_depth() {
    let temp = tempfile::tempdir().unwrap();
    basic(temp.path(), "root", &[]);
    fs::OpenOptions::new()
        .write(true)
        .open(temp.path().join("flow.json"))
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    assert!(
        capture(temp.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("byte limit")
    );
    let deep = format!("{}file.bin", "nested/".repeat(66));
    package(
        temp.path(),
        "root",
        &[("flow.rs", b"root"), (&deep, b"asset")],
        &[],
    );
    assert!(
        capture(temp.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("depth")
    );
}
