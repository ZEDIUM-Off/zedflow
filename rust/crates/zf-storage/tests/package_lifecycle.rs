use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use zf_flows::package::PackageSnapshot;
use zf_storage::flow_packages::{
    self, BridgeMutation, LegacyRetirement, PackageConversion, PackageDeletion,
};

fn snapshot(id: &str, source: &[u8]) -> PackageSnapshot {
    PackageSnapshot::capture(serde_json::json!({"formatVersion":1,"id":id,"name":id,"entry":"flow.rs","files":["flow.rs"]}).to_string(),
        BTreeMap::from([("flow.rs".into(), source.to_vec())]), BTreeMap::new()).unwrap()
}
fn file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
async fn request(
    workspace: &Path,
    roots: Vec<PathBuf>,
    bridges: Vec<BridgeMutation>,
) -> PackageConversion {
    let legacy = workspace.join(".agents/flows/nested/sample.rs");
    let source = b"// original source\n";
    file(&legacy, source);
    PackageConversion {
        workspace: workspace.into(),
        snapshot: snapshot("sample", source),
        legacy: LegacyRetirement {
            path: legacy,
            before: source.to_vec(),
        },
        bridges,
        package_preconditions: vec![],
        preconditions: flow_packages::capture_catalogue_preconditions(roots)
            .await
            .unwrap(),
    }
}
fn assert_clean(workspace: &Path) {
    for name in [
        ".package-lifecycle.json",
        ".package-lifecycle-participant.json",
    ] {
        assert!(
            !workspace.join(".zedflow").join(name).exists(),
            "{name} remains"
        );
    }
}

fn interrupted_conversion(
    write: &PackageConversion,
    id: &str,
    phase: &str,
    participants: &[PathBuf],
) {
    file(&write.workspace.join(".zedflow/.package-lifecycle.json"), &serde_json::to_vec(&serde_json::json!({
        "version":1,"id":id,"workspace":write.workspace,"participants":participants,
        "preconditions":write.preconditions,"packagePreconditions":write.package_preconditions,"operation":{"kind":"convert","snapshot":write.snapshot,"legacy":write.legacy,"bridges":write.bridges},"phase":phase
    })).unwrap());
    for root in participants {
        if *root != write.workspace {
            file(
                &root.join(".zedflow/.package-lifecycle-participant.json"),
                &serde_json::to_vec(
                    &serde_json::json!({"version":1,"id":id,"coordinator":write.workspace}),
                )
                .unwrap(),
            );
        }
    }
}
fn private(root: &Path, id: &str, suffix: &str) -> PathBuf {
    root.join(".zedflow")
        .join(format!(".package-lifecycle-{id}-{suffix}"))
}
fn materialize(root: &Path, snapshot: &PackageSnapshot) {
    let node = snapshot.root_node().unwrap();
    file(&root.join("flow.json"), node.manifest_source.as_bytes());
    for (path, bytes) in &node.files {
        file(&root.join(path), bytes);
    }
}

#[tokio::test]
async fn recovery_finishes_partial_conversion_from_a_participant() {
    for renamed_package in [false, true] {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let path = b.path().join(".zedflow/bridges/use.rs");
        file(&path, b"before");
        let mut roots = vec![a.path().into(), b.path().into()];
        roots.sort();
        let write = request(
            a.path(),
            roots.clone(),
            vec![BridgeMutation {
                workspace: b.path().into(),
                path: path.clone(),
                before: b"before".to_vec(),
                after: b"after".to_vec(),
            }],
        )
        .await;
        let id = uuid::Uuid::new_v4().to_string();
        interrupted_conversion(&write, &id, "ready", &roots);
        materialize(&private(a.path(), &id, "package"), &write.snapshot);
        file(&private(b.path(), &id, "bridge-0"), b"after");
        fs::rename(&write.legacy.path, private(a.path(), &id, "legacy")).unwrap();
        if renamed_package {
            fs::create_dir_all(a.path().join(".zedflow/flow")).unwrap();
            fs::rename(
                private(a.path(), &id, "package"),
                a.path().join(".zedflow/flow/sample"),
            )
            .unwrap();
        }
        assert!(
            flow_packages::recover_lifecycle(b.path().into())
                .await
                .unwrap()
        );
        assert_eq!(fs::read(path).unwrap(), b"after");
        assert_eq!(
            flow_packages::capture(&a.path().join(".zedflow/flow/sample"))
                .await
                .unwrap(),
            write.snapshot
        );
        assert!(!write.legacy.path.exists());
        assert_clean(a.path());
        assert_clean(b.path());
    }
}

#[tokio::test]
async fn finishing_recovery_preserves_edits_after_a_participant_was_released() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let path = b.path().join(".zedflow/bridges/use.rs");
    file(&path, b"before");
    let mut roots = vec![a.path().into(), b.path().into()];
    roots.sort();
    let write = request(
        a.path(),
        roots.clone(),
        vec![BridgeMutation {
            workspace: b.path().into(),
            path: path.clone(),
            before: b"before".to_vec(),
            after: b"after".to_vec(),
        }],
    )
    .await;
    let id = uuid::Uuid::new_v4().to_string();
    interrupted_conversion(&write, &id, "finishing", &roots);
    materialize(&a.path().join(".zedflow/flow/sample"), &write.snapshot);
    fs::remove_file(&write.legacy.path).unwrap(); // Its backup was already cleaned.
    file(&path, b"after");
    fs::remove_file(
        b.path()
            .join(".zedflow/.package-lifecycle-participant.json"),
    )
    .unwrap();
    // A completed phase is durable. A later edit must not require restoring the
    // old bridge just so the coordinator can remove its final cleanup marker.
    file(&path, b"later edit");
    assert!(
        flow_packages::recover_lifecycle(a.path().into())
            .await
            .unwrap()
    );
    assert_eq!(fs::read(path).unwrap(), b"later edit");
    assert_clean(a.path());
    assert_clean(b.path());
}

#[tokio::test]
async fn recovery_preserves_tampered_retirement_backup_and_blocks_catalogue_reads() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut roots = vec![a.path().into(), b.path().into()];
    roots.sort();
    let write = request(a.path(), roots.clone(), vec![]).await;
    let id = uuid::Uuid::new_v4().to_string();
    interrupted_conversion(&write, &id, "ready", &roots);
    materialize(&private(a.path(), &id, "package"), &write.snapshot);
    fs::rename(&write.legacy.path, private(a.path(), &id, "legacy")).unwrap();
    file(&private(a.path(), &id, "legacy"), b"external edit");
    assert!(
        flow_packages::recover_lifecycle(b.path().into())
            .await
            .is_err()
    );
    assert_eq!(
        fs::read(private(a.path(), &id, "legacy")).unwrap(),
        b"external edit"
    );
    assert!(
        zf_storage::context_store::SourceStore::new(b.path().into(), &["bridges"])
            .unwrap()
            .list()
            .await
            .is_err()
    );
    assert!(
        zf_storage::source_acceptance::recover(b.path().into())
            .await
            .is_err()
    );
    assert!(flow_packages::recover(b.path().into()).await.is_err());
}

#[tokio::test]
async fn conversion_retires_legacy_and_updates_multiple_workspace_consumers() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let path = b.path().join(".zedflow/bridges/use.rs");
    file(&path, b"old bridge");
    let mutation = BridgeMutation {
        workspace: b.path().into(),
        path: path.clone(),
        before: b"old bridge".to_vec(),
        after: b"new bridge".to_vec(),
    };
    let write = request(
        a.path(),
        vec![a.path().into(), b.path().into()],
        vec![mutation],
    )
    .await;
    let expected = write.snapshot.clone();
    let old = write.legacy.path.clone();
    assert_eq!(
        flow_packages::convert_package(write).await.unwrap(),
        expected
    );
    assert!(!old.exists());
    assert_eq!(fs::read(path).unwrap(), b"new bridge");
    assert_eq!(
        flow_packages::capture(&a.path().join(".zedflow/flow/sample"))
            .await
            .unwrap(),
        expected
    );
    assert_clean(a.path());
    assert_clean(b.path());
    assert!(
        !flow_packages::recover_lifecycle(b.path().into())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn conversion_detects_new_consumers_after_audit_before_any_retirement() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let write = request(a.path(), vec![a.path().into(), b.path().into()], vec![]).await;
    let legacy = write.legacy.path.clone();
    file(&b.path().join(".zedflow/bridges/new.rs"), b"new consumer");
    assert!(flow_packages::convert_package(write).await.is_err());
    assert_eq!(fs::read(legacy).unwrap(), b"// original source\n");
    assert!(!a.path().join(".zedflow/flow/sample").exists());
    assert_clean(a.path());
    assert_clean(b.path());
}

#[tokio::test]
async fn deletion_uses_full_revision_and_preserves_unexpected_inventory() {
    let a = tempfile::tempdir().unwrap();
    let write = request(a.path(), vec![a.path().into()], vec![]).await;
    let expected = flow_packages::convert_package(write).await.unwrap();
    let target = a.path().join(".zedflow/flow/sample");
    let preconditions = flow_packages::capture_catalogue_preconditions(vec![a.path().into()])
        .await
        .unwrap();
    file(&target.join("external.txt"), b"preserve");
    assert!(
        flow_packages::delete_package(PackageDeletion {
            workspace: a.path().into(),
            target: target.clone(),
            expected_revision: expected.root.clone(),
            preconditions,
            package_preconditions: vec![]
        })
        .await
        .is_err()
    );
    assert_eq!(fs::read(target.join("external.txt")).unwrap(), b"preserve");
    fs::remove_file(target.join("external.txt")).unwrap();
    let preconditions = flow_packages::capture_catalogue_preconditions(vec![a.path().into()])
        .await
        .unwrap();
    flow_packages::delete_package(PackageDeletion {
        workspace: a.path().into(),
        target: target.clone(),
        expected_revision: expected.root,
        preconditions,
        package_preconditions: vec![],
    })
    .await
    .unwrap();
    assert!(!target.exists());
    assert_clean(a.path());
}

async fn interrupted_deletion(root: &Path, phase: &str) -> (String, PathBuf, PathBuf) {
    let write = request(root, vec![root.into()], vec![]).await;
    let snapshot = flow_packages::convert_package(write).await.unwrap();
    let target = root.join(".zedflow/flow/sample");
    let conditions = flow_packages::capture_catalogue_preconditions(vec![root.into()])
        .await
        .unwrap();
    let values = serde_json::to_value(&conditions).unwrap();
    let catalogue = values
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["catalogue"] == ".zedflow/flow")
        .unwrap();
    let inventory: BTreeMap<String, serde_json::Value> = catalogue["inventory"]
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("sample/")
                .map(|key| (key.into(), value.clone()))
        })
        .collect();
    let id = uuid::Uuid::new_v4().to_string();
    file(&root.join(".zedflow/.package-lifecycle.json"),&serde_json::to_vec(&serde_json::json!({
        "version":1,"id":id,"workspace":root,"participants":[root],"preconditions":conditions,"packagePreconditions":[],
        "operation":{"kind":"delete","snapshot":snapshot,"inventory":inventory},"phase":phase
    })).unwrap());
    let stage = private(root, &id, "package");
    (id, target, stage)
}

#[tokio::test]
async fn deletion_recovers_on_either_side_of_quarantine_rename() {
    for quarantined in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (_, target, stage) = interrupted_deletion(root.path(), "ready").await;
        if quarantined {
            fs::rename(&target, &stage).unwrap();
        }
        assert!(
            flow_packages::recover_lifecycle(root.path().into())
                .await
                .unwrap()
        );
        assert!(!target.exists());
        assert!(!stage.exists());
        assert_clean(root.path());
        assert!(
            !flow_packages::recover_lifecycle(root.path().into())
                .await
                .unwrap()
        );
    }
}

#[tokio::test]
async fn deletion_finishes_partial_cleanup_without_touching_recreated_package() {
    let root = tempfile::tempdir().unwrap();
    let (_, target, stage) = interrupted_deletion(root.path(), "finishing").await;
    fs::rename(&target, &stage).unwrap();
    fs::remove_file(stage.join("flow.rs")).unwrap();
    file(&target.join("new.txt"), b"new independent contents");
    assert!(
        flow_packages::recover_lifecycle(root.path().into())
            .await
            .unwrap()
    );
    assert_eq!(
        fs::read(target.join("new.txt")).unwrap(),
        b"new independent contents"
    );
    assert!(!stage.exists());
    assert_clean(root.path());
}

#[tokio::test]
async fn deletion_preserves_changed_quarantine_before_and_during_cleanup() {
    for phase in ["ready", "installed", "finishing"] {
        let root = tempfile::tempdir().unwrap();
        let (_, target, stage) = interrupted_deletion(root.path(), phase).await;
        fs::rename(&target, &stage).unwrap();
        file(&stage.join("external.txt"), b"preserve");
        assert!(
            flow_packages::recover_lifecycle(root.path().into())
                .await
                .is_err()
        );
        assert_eq!(fs::read(stage.join("external.txt")).unwrap(), b"preserve");
        assert!(stage.join("flow.rs").exists());
        assert!(
            root.path()
                .join(".zedflow/.package-lifecycle.json")
                .exists()
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn deletion_rejects_replaced_quarantine_symlink_without_touching_destination() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let (_, target, stage) = interrupted_deletion(root.path(), "finishing").await;
    fs::rename(target, &stage).unwrap();
    fs::remove_file(stage.join("flow.rs")).unwrap();
    file(&outside.path().join("source.rs"), b"private external file");
    std::os::unix::fs::symlink(outside.path().join("source.rs"), stage.join("flow.rs")).unwrap();
    assert!(
        flow_packages::recover_lifecycle(root.path().into())
            .await
            .is_err()
    );
    assert_eq!(
        fs::read(outside.path().join("source.rs")).unwrap(),
        b"private external file"
    );
    assert!(
        fs::symlink_metadata(stage.join("flow.rs"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[tokio::test]
async fn conversion_revalidates_external_dependency_closures_before_retirement_and_recovery() {
    for recovering in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let dependency = root.path().join("external");
        materialize(&dependency, &snapshot("external", b"original dependency"));
        let consumer = root.path().join(".zedflow/flow/consumer");
        file(&consumer.join("flow.json"),serde_json::json!({
            "formatVersion":1,"id":"consumer","name":"consumer","entry":"flow.rs","files":["flow.rs"],
            "dependencies":{"shared":{"path":"../../../external"}}
        }).to_string().as_bytes());
        file(&consumer.join("flow.rs"), b"consumer");
        let capture = flow_packages::capture(&consumer).await.unwrap();
        let mut write = request(root.path(), vec![root.path().into()], vec![]).await;
        write
            .package_preconditions
            .push(flow_packages::PackagePrecondition {
                path: consumer,
                revision: capture.root,
            });
        let legacy = write.legacy.path.clone();
        if recovering {
            let id = uuid::Uuid::new_v4().to_string();
            interrupted_conversion(&write, &id, "ready", &[root.path().into()]);
            materialize(&private(root.path(), &id, "package"), &write.snapshot);
        }
        // This file lies outside every recorded catalogue. Only full closure
        // reacquisition detects the changed audit evidence.
        file(&dependency.join("flow.rs"), b"changed dependency");
        let error = if recovering {
            flow_packages::recover_lifecycle(root.path().into())
                .await
                .unwrap_err()
        } else {
            flow_packages::convert_package(write).await.unwrap_err()
        };
        assert!(
            format!("{error:#}").contains("dependency closure changed"),
            "{error:#}"
        );
        assert_eq!(fs::read(legacy).unwrap(), b"// original source\n");
        assert!(!root.path().join(".zedflow/flow/sample").exists());
    }
}

#[tokio::test]
async fn staging_recovers_owned_partial_fragments_outside_the_package() {
    for participant in [false, true] {
        for contents in [b"".as_slice(), b"{".as_slice(), b"foreign".as_slice()] {
            let a = tempfile::tempdir().unwrap();
            let b = tempfile::tempdir().unwrap();
            let mut roots = vec![a.path().into(), b.path().into()];
            roots.sort();
            let write = request(a.path(), roots.clone(), vec![]).await;
            let id = uuid::Uuid::new_v4().to_string();
            interrupted_conversion(&write, &id, "staging", &roots);
            let destination = if participant {
                let marker = b
                    .path()
                    .join(".zedflow/.package-lifecycle-participant.json");
                fs::remove_file(&marker).unwrap();
                marker
            } else {
                private(a.path(), &id, "package").join("flow.json")
            };
            let hash = zf_storage::context_store::hash(destination.to_str().unwrap().as_bytes());
            let fragment = private(
                if participant { b.path() } else { a.path() },
                &id,
                &format!("fragment-{hash}"),
            );
            file(&fragment, contents);
            let result = flow_packages::recover_lifecycle(a.path().into()).await;
            if contents == b"foreign" {
                assert!(result.is_err());
                assert_eq!(fs::read(&fragment).unwrap(), contents);
                assert!(write.legacy.path.exists());
            } else {
                assert!(result.unwrap());
                assert!(!fragment.exists());
                assert!(!write.legacy.path.exists());
                assert_clean(a.path());
                assert_clean(b.path());
            }
        }
    }
}

#[tokio::test]
async fn deleting_package_preserves_its_external_dependencies() {
    let root = tempfile::tempdir().unwrap();
    let external = root.path().join("external");
    materialize(&external, &snapshot("external", b"external content"));
    let package = root.path().join(".zedflow/flow/consumer");
    file(&package.join("flow.json"),serde_json::json!({
        "formatVersion":1,"id":"consumer","name":"consumer","entry":"flow.rs","files":["flow.rs"],
        "dependencies":{"shared":{"path":"../../../external"}}
    }).to_string().as_bytes());
    file(&package.join("flow.rs"), b"consumer");
    let snapshot = flow_packages::capture(&package).await.unwrap();
    let preconditions = flow_packages::capture_catalogue_preconditions(vec![root.path().into()])
        .await
        .unwrap();
    flow_packages::delete_package(PackageDeletion {
        workspace: root.path().into(),
        target: package.clone(),
        expected_revision: snapshot.root,
        preconditions,
        package_preconditions: vec![],
    })
    .await
    .unwrap();
    assert!(!package.exists());
    assert_eq!(
        fs::read(external.join("flow.rs")).unwrap(),
        b"external content"
    );
    assert!(flow_packages::capture(&external).await.is_ok());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn conversion_publishes_participants_on_separate_filesystems() {
    use std::os::unix::fs::MetadataExt;
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(
        fs::metadata(a.path()).unwrap().dev(),
        fs::metadata(b.path()).unwrap().dev(),
        "test requires separate filesystems"
    );
    let bridge = b.path().join(".zedflow/bridges/use.rs");
    file(&bridge, b"before");
    let write = request(
        a.path(),
        vec![a.path().into(), b.path().into()],
        vec![BridgeMutation {
            workspace: b.path().into(),
            path: bridge.clone(),
            before: b"before".to_vec(),
            after: b"after".to_vec(),
        }],
    )
    .await;
    flow_packages::convert_package(write).await.unwrap();
    assert_eq!(fs::read(bridge).unwrap(), b"after");
    assert_clean(a.path());
    assert_clean(b.path());
}
