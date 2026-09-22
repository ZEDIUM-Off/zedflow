//! Catalog boundaries for package roots and the four historical source roots.
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use zf_flows::{flow_source, schema::Composition};
use zf_storage::{
    flow_store::FlowStore,
    workspaces::{Workspace, path_id},
};

fn document(id: &str) -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":id,"name":id,
        "nodes":[{"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
        {"id":"end","position":{"x":100,"y":0},"data":{"kind":"end","label":"End","config":{}}}],
        "edges":[{"id":"done","source":"start","target":"end"}]})).unwrap()
}
fn source(id: &str) -> String {
    flow_source::render(&document(id), &|_: &Composition| Ok(())).unwrap()
}
fn package(parent: &Path, id: &str) -> PathBuf {
    let path = parent.join(".zedflow/flow").join(id);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join("flow.json"),
        serde_json::to_vec(&json!({
            "formatVersion":1,"id":id,"name":id,"entry":"flow.rs", "files":["flow.rs","README.md"]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(path.join("flow.rs"), source(id)).unwrap();
    std::fs::write(path.join("README.md"), "original").unwrap();
    path
}
fn setup(root: &Path) -> (Workspace, PathBuf, FlowStore) {
    let workspace = root.join("workspace");
    let home = root.join("home");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let store = FlowStore::new(home.clone(), Arc::new(|_: &Composition| Ok(())));
    (
        Workspace {
            id: path_id(&workspace),
            name: "fixture".into(),
            path: workspace,
            open: true,
        },
        home,
        store,
    )
}

#[tokio::test]
async fn list_is_lightweight_but_get_freezes_every_declared_byte_and_revision() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let path = package(&workspace.path, "sample");
    let files = store.list(&workspace).await.unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        files[0].diagnostics.is_empty(),
        "{:?}",
        files[0].diagnostics
    );
    assert!(files[0].package.is_none());
    assert!(files[0].source.is_none());
    let before = store.get(&workspace, &path_id(&path)).await.unwrap();
    assert_eq!(before.hash, before.package.as_ref().unwrap().root);
    assert_eq!(
        before.source_hash,
        zf_storage::flow_store::hash(source("sample").as_bytes())
    );
    assert_ne!(before.hash, before.source_hash);
    assert_eq!(before.preconditions.len(), 3);
    std::fs::write(path.join("README.md"), "changed").unwrap();
    let after = store.get(&workspace, &before.key).await.unwrap();
    assert_ne!(before.hash, after.hash);
    assert_eq!(before.source_hash, after.source_hash);
    assert_eq!(
        before.package.unwrap().root_node().unwrap().files["README.md"],
        b"original"
    );
}

#[tokio::test]
async fn unsupported_visual_rust_preserves_package_capture_and_blocks_visual_save() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let path = package(&workspace.path, "sample");
    let entry = format!("mod helper;\n{}", source("sample"));
    let helper = b"pub const MESSAGE: &str = \"preserve me\";\n";
    let manifest = serde_json::to_vec(&json!({
        "formatVersion": 1, "id": "sample", "name": "Package name",
        "entry": "flow.rs", "files": ["flow.rs", "README.md", "helper.rs"]
    }))
    .unwrap();
    std::fs::write(path.join("flow.json"), &manifest).unwrap();
    std::fs::write(path.join("flow.rs"), &entry).unwrap();
    std::fs::write(path.join("helper.rs"), helper).unwrap();

    let listed = store.list(&workspace).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].package.is_none());
    assert!(listed[0].source.is_none());
    assert!(!listed[0].diagnostics.is_empty());
    let loaded = store.get(&workspace, &path_id(&path)).await.unwrap();
    assert_eq!(loaded.id, "sample");
    assert_eq!(loaded.name, "Package name");
    assert_eq!(loaded.source.as_deref(), Some(entry.as_str()));
    assert_eq!(
        loaded.source_hash,
        zf_storage::flow_store::hash(entry.as_bytes())
    );
    assert_eq!(loaded.hash, listed[0].hash);
    assert_eq!(loaded.preconditions.len(), 4);
    assert!(loaded.composition.is_none());
    assert!(!loaded.diagnostics.is_empty());
    let snapshot = loaded.package.as_ref().unwrap();
    assert_eq!(loaded.hash, snapshot.root);
    let root = snapshot.root_node().unwrap();
    assert_eq!(root.manifest_source.as_bytes(), manifest);
    assert_eq!(root.files["flow.rs"], entry.as_bytes());
    assert_eq!(root.files["helper.rs"], helper);
    assert_eq!(root.files["README.md"], b"original");
    let error = store
        .store(
            &workspace,
            document("sample"),
            "workspace",
            Some(&loaded.key),
            Some(&loaded.hash),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("diagnostics"));
    assert_eq!(
        std::fs::read(path.join("flow.rs")).unwrap(),
        entry.as_bytes()
    );
    assert_eq!(std::fs::read(path.join("helper.rs")).unwrap(), helper);
    // A visually unsupported package still reserves its manifest identity.
    let legacy = workspace.path.join(".zedflow/flows/sample.rs");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, source("sample")).unwrap();
    let collisions = store.list(&workspace).await.unwrap();
    assert_eq!(collisions.len(), 2);
    assert!(
        collisions
            .iter()
            .all(|file| file.diagnostics.iter().any(|d| d.contains("concurrente")))
    );
}

#[tokio::test]
async fn legacy_and_package_identity_collisions_are_visible_instead_of_priority_ordered() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let path = package(&workspace.path, "same");
    let legacy = workspace.path.join(".agents/flows");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("same.rs"), source("same")).unwrap();
    let files = store.list(&workspace).await.unwrap();
    assert_eq!(files.len(), 2);
    assert!(
        files
            .iter()
            .all(|file| file.diagnostics.iter().any(|s| s.contains("concurrente")))
    );
    let loaded = store.get(&workspace, &path_id(&path)).await.unwrap();
    assert!(loaded.diagnostics.iter().any(|s| s.contains("concurrente")));
}

#[tokio::test]
async fn global_packages_and_all_legacy_roots_remain_discoverable() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, home, store) = setup(temp.path());
    package(&home, "global");
    for (index, base) in [&workspace.path, &home].into_iter().enumerate() {
        for (j, relative) in [".zedflow/flows", ".agents/flows"].into_iter().enumerate() {
            let path = base.join(relative);
            std::fs::create_dir_all(&path).unwrap();
            let id = format!("old-{index}-{j}");
            std::fs::write(path.join(format!("{id}.rs")), source(&id)).unwrap();
        }
    }
    let files = store.list(&workspace).await.unwrap();
    assert_eq!(files.len(), 5);
    assert!(files.iter().all(|file| file.diagnostics.is_empty()));
    assert_eq!(
        files.iter().filter(|file| file.scope == "global").count(),
        3
    );
}

#[tokio::test]
async fn invalid_manifest_identity_and_undeclared_files_are_catalog_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let path = package(&workspace.path, "sample");
    std::fs::write(path.join("flow.rs"), source("other")).unwrap();
    let files = store.list(&workspace).await.unwrap();
    assert!(files[0].diagnostics.iter().any(|s| s.contains("identity")));
    std::fs::write(path.join("flow.rs"), source("sample")).unwrap();
    std::fs::write(path.join("unexpected.txt"), "not declared").unwrap();
    let files = store.list(&workspace).await.unwrap();
    assert!(!files[0].diagnostics.is_empty());
    assert!(files[0].composition.is_none());
}

#[tokio::test]
async fn new_writer_uses_package_directories_and_preserves_secondary_files_on_edit() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let created = store
        .store(&workspace, document("created"), "workspace", None, None)
        .await
        .unwrap();
    assert_eq!(created.path, workspace.path.join(".zedflow/flow/created"));
    assert!(created.path.join("flow.json").is_file());
    assert!(!workspace.path.join(".zedflow/flows").exists());
    let path = package(&workspace.path, "sample");
    let original = store.get(&workspace, &path_id(&path)).await.unwrap();
    let mut doc = original.composition.clone().unwrap();
    doc.name = "Changed name".into();
    let updated = store
        .store(
            &workspace,
            doc,
            "workspace",
            Some(&original.key),
            Some(&original.hash),
        )
        .await
        .unwrap();
    assert_ne!(updated.hash, original.hash);
    assert_eq!(std::fs::read(path.join("README.md")).unwrap(), b"original");
    assert_eq!(
        updated.package.unwrap().root_manifest().unwrap().name,
        "Changed name"
    );
    let stale = store
        .store(
            &workspace,
            document("sample"),
            "workspace",
            Some(&original.key),
            Some(&original.hash),
        )
        .await;
    assert!(stale.is_err());
}

#[tokio::test]
async fn editing_legacy_requires_explicit_conversion_and_keeps_source_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, _, store) = setup(temp.path());
    let path = workspace.path.join(".zedflow/flows/legacy.rs");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = source("legacy");
    std::fs::write(&path, &original).unwrap();
    let file = store.get(&workspace, &path_id(&path)).await.unwrap();
    let error = store
        .store(
            &workspace,
            document("legacy"),
            "workspace",
            Some(&file.key),
            Some(&file.hash),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("converti explicitement"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
    assert!(!workspace.path.join(".zedflow/flow/legacy").exists());
}

#[tokio::test]
async fn conversion_proposals_preserve_exact_source_from_each_legacy_root_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, home, store) = setup(temp.path());
    for (index, base) in [&workspace.path, &home].into_iter().enumerate() {
        for (namespace, relative) in [".zedflow/flows", ".agents/flows"].into_iter().enumerate() {
            let id = format!("legacy-{index}-{namespace}");
            let path = base.join(relative).join("nested").join(format!("{id}.rs"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let original = format!("{}\n// Commentaire conservé — exact\n", source(&id));
            std::fs::write(&path, &original).unwrap();
            let expected = zf_storage::flow_store::hash(original.as_bytes());
            let plan = store
                .plan_conversion(&workspace, &path_id(&path), &expected)
                .await
                .unwrap();
            assert_eq!(plan.lock_workspace, *base);
            assert_eq!(plan.target, base.join(".zedflow/flow").join(&id));
            assert_eq!(
                plan.package.root_node().unwrap().entry_source().unwrap(),
                original
            );
            assert_eq!(plan.package.root_manifest().unwrap().id.as_str(), id);
            assert_eq!(plan.legacy.hash, expected);
            assert_ne!(plan.package.root, expected);
            assert!(!plan.target.exists());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
            assert!(
                store
                    .plan_conversion(&workspace, &path_id(&path), "stale")
                    .await
                    .is_err()
            );
        }
    }
}

#[tokio::test]
async fn conversion_proposal_refuses_same_identity_even_when_legacy_bytes_match() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, home, store) = setup(temp.path());
    let original = source("same");
    let local = workspace.path.join(".zedflow/flows/same.rs");
    let global = home.join(".agents/flows/same.rs");
    for path in [&local, &global] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &original).unwrap();
    }
    let error = store
        .plan_conversion(
            &workspace,
            &path_id(&local),
            &zf_storage::flow_store::hash(original.as_bytes()),
        )
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("concurrente"));
    assert_eq!(std::fs::read_to_string(local).unwrap(), original);
    assert_eq!(std::fs::read_to_string(global).unwrap(), original);
    assert!(!workspace.path.join(".zedflow/flow").exists());
}

#[tokio::test]
async fn hydrated_catalog_matches_get_and_list_including_invalid_and_duplicate_entries() {
    let temp = tempfile::tempdir().unwrap();
    let (workspace, home, store) = setup(temp.path());
    package(&workspace.path, "same");
    package(&home, "global");
    let legacy = workspace.path.join(".agents/flows");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("same.rs"), source("same")).unwrap();
    std::fs::write(legacy.join("valid.rs"), source("valid")).unwrap();
    std::fs::write(legacy.join("broken.rs"), "malformed Rust").unwrap();
    let unsupported = package(&workspace.path, "unsupported");
    std::fs::write(unsupported.join("flow.rs"), "unsupported Rust").unwrap();
    let malformed = workspace.path.join(".zedflow/flow/malformed");
    std::fs::create_dir_all(&malformed).unwrap();
    std::fs::write(malformed.join("flow.json"), "malformed JSON").unwrap();

    let captured = store.capture_catalog(&workspace).await.unwrap();
    let listed = store.list(&workspace).await.unwrap();
    assert_eq!(captured.len(), 7);
    assert_eq!(listed.len(), captured.len());
    assert_eq!(
        captured
            .iter()
            .filter(|f| !f.diagnostics.is_empty())
            .count(),
        5
    );
    for (file, summary) in captured.iter().zip(&listed) {
        let loaded = store.get(&workspace, &file.key).await.unwrap();
        assert_eq!(
            serde_json::to_value(file).unwrap(),
            serde_json::to_value(&loaded).unwrap()
        );
        assert_eq!(
            file.preconditions
                .iter()
                .map(|p| (&p.path, &p.hash))
                .collect::<Vec<_>>(),
            loaded
                .preconditions
                .iter()
                .map(|p| (&p.path, &p.hash))
                .collect::<Vec<_>>()
        );
        let mut light = file.clone();
        light.source = None;
        light.package = None;
        assert_eq!(
            serde_json::to_value(light).unwrap(),
            serde_json::to_value(summary).unwrap()
        );
    }
    assert!(
        captured
            .iter()
            .filter(|f| f.id == "same")
            .all(|f| f.diagnostics.iter().any(|d| d.contains("concurrente")))
    );
}

#[tokio::test]
async fn repeated_sources_revalidate_capabilities_and_external_bytes() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let temp = tempfile::tempdir().unwrap();
    let (workspace, home, _) = setup(temp.path());
    let allowed = Arc::new(AtomicBool::new(true));
    let calls = Arc::new(AtomicUsize::new(0));
    let store = FlowStore::new(
        home,
        Arc::new({
            let allowed = allowed.clone();
            let calls = calls.clone();
            move |_: &Composition| {
                calls.fetch_add(1, Ordering::SeqCst);
                anyhow::ensure!(allowed.load(Ordering::SeqCst), "fixture capability revoked");
                Ok(())
            }
        }),
    );
    let package = package(&workspace.path, "sample");
    assert!(
        store.list(&workspace).await.unwrap()[0]
            .diagnostics
            .is_empty()
    );
    let warmed = calls.load(Ordering::SeqCst);
    assert!(
        store.list(&workspace).await.unwrap()[0]
            .diagnostics
            .is_empty()
    );
    assert!(calls.load(Ordering::SeqCst) > warmed);
    allowed.store(false, Ordering::SeqCst);
    let rejected = store.list(&workspace).await.unwrap();
    assert!(
        rejected[0]
            .diagnostics
            .iter()
            .any(|d| d.contains("fixture capability revoked"))
    );
    allowed.store(true, Ordering::SeqCst);
    assert!(
        store.list(&workspace).await.unwrap()[0]
            .diagnostics
            .is_empty()
    );
    // Same path and identity, changed unsupported Rust: a warmed source must
    // never mask a new body. Package capture itself remains inspectable.
    std::fs::write(
        package.join("flow.rs"),
        format!("{}\nfn hidden_effect() {{}}", source("sample")),
    )
    .unwrap();
    let changed = store.capture_catalog(&workspace).await.unwrap();
    assert!(!changed[0].diagnostics.is_empty());
    assert!(changed[0].composition.is_none());
    assert!(changed[0].package.is_some());
    std::fs::write(package.join("flow.rs"), source("sample")).unwrap();
    assert!(
        store.list(&workspace).await.unwrap()[0]
            .diagnostics
            .is_empty()
    );
}
