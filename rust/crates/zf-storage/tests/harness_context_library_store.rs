use serde_json::json;
use std::collections::BTreeMap;
use zf_context::context::ContextExpr;
use zf_context::context::ContextFunction;
use zf_context::context::ContextLibrary;
use zf_context::context_source::generate_library;
use zf_context::context_source::parse_library;
use zf_core::types::DataType;
use zf_storage::context_store::Conflict;
use zf_storage::context_store::ContextStore;
use zf_storage::context_store::LibraryStore;
use zf_storage::context_store::SourceStore;
fn library(value: &str) -> ContextLibrary {
    ContextLibrary::new().projection(
        "label",
        ContextFunction::new(
            BTreeMap::new(),
            DataType::Text,
            ContextExpr::literal(DataType::Text, json!(value)),
        ),
    )
}
#[tokio::test]
async fn libraries_are_rust_sources_with_invalid_entries_visible_and_conflicts_preserved() {
    let root = tempfile::tempdir().unwrap();
    let store = LibraryStore::new(root.path().into());
    let original = store
        .save("display", &library("first"), None)
        .await
        .unwrap();
    assert!(
        original
            .path
            .starts_with(root.path().join(".zedflow/context/libraries"))
    );
    assert_eq!(original.library, Some(library("first")));
    assert_eq!(
        parse_library(original.source.as_ref().unwrap()).unwrap(),
        library("first")
    );
    assert_eq!(
        std::fs::read_to_string(&original.path).unwrap(),
        generate_library(&library("first")).unwrap()
    );
    assert!(
        ContextStore::new(root.path().into())
            .list()
            .await
            .unwrap()
            .is_empty()
    );
    let edited = generate_library(&library("external")).unwrap();
    std::fs::write(&original.path, &edited).unwrap();
    let conflict = store
        .save("display", &library("overwrite"), Some(&original.hash))
        .await
        .unwrap_err();
    assert!(conflict.downcast_ref::<Conflict>().is_some());
    assert_eq!(std::fs::read_to_string(&original.path).unwrap(), edited);
    std::fs::write(
        original.path.parent().unwrap().join("broken.rs"),
        "fn arbitrary() { panic!(); }",
    )
    .unwrap();
    let list = store.list().await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|f| f.source.is_none()));
    let broken = list.iter().find(|f| f.key == "broken").unwrap();
    assert!(broken.library.is_none());
    assert!(!broken.diagnostics.is_empty());
    assert!(store.read("../display").await.is_err());
}
#[tokio::test]
async fn library_writers_commit_only_one_revision_and_share_the_generic_writer() {
    let root = tempfile::tempdir().unwrap();
    let left = LibraryStore::new(root.path().into());
    let right = LibraryStore::new(root.path().into());
    let original = left.save("display", &library("first"), None).await.unwrap();
    let a = library("left");
    let b = library("right");
    let (a, b) = tokio::join!(
        left.save("display", &a, Some(&original.hash)),
        right.save("display", &b, Some(&original.hash))
    );
    let saved = match (a, b) {
        (Ok(saved), Err(error)) | (Err(error), Ok(saved)) => {
            assert!(error.downcast_ref::<Conflict>().is_some());
            saved
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(left.read("display").await.unwrap().hash, saved.hash);
    let raw = SourceStore::new(root.path().into(), &["bridges"]).unwrap();
    let bridge = raw
        .save("shared", "// a codec-validated Rust document", None)
        .await
        .unwrap();
    assert!(
        bridge
            .path
            .starts_with(root.path().join(".zedflow/bridges"))
    );
    assert_eq!(raw.list().await.unwrap()[0].source, bridge.source);
    for segments in [vec![".."], vec!["context/libraries"], vec!["/tmp"], vec![]] {
        assert!(SourceStore::new(root.path().into(), &segments).is_err());
    }
}
#[cfg(unix)]
#[tokio::test]
async fn library_directories_and_sources_never_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".zedflow/context")).unwrap();
    symlink(
        outside.path(),
        root.path().join(".zedflow/context/libraries"),
    )
    .unwrap();
    let store = LibraryStore::new(root.path().into());
    assert!(store.list().await.is_err());
    assert!(store.save("display", &library("new"), None).await.is_err());
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    std::fs::remove_file(root.path().join(".zedflow/context/libraries")).unwrap();
    let file = store.save("display", &library("new"), None).await.unwrap();
    let target = outside.path().join("source.rs");
    std::fs::write(&target, file.source.as_ref().unwrap()).unwrap();
    std::fs::remove_file(&file.path).unwrap();
    symlink(&target, &file.path).unwrap();
    assert!(!store.read("display").await.unwrap().diagnostics.is_empty());
    assert!(
        store
            .save("display", &library("change"), Some(&file.hash))
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        file.source.unwrap()
    );
}
