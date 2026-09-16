use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use zf_flows::package::PackageSnapshot;
use zf_storage::flow_packages::{self, PackageWrite};

fn snapshot(id: &str, source: &str, secondary: &str) -> PackageSnapshot {
    PackageSnapshot::capture(
        format!(r#"{{"formatVersion":1,"id":"{id}","name":"Same Cargo name","entry":"flow.rs","files":["flow.rs","modules/extra.rs"]}}"#),
        BTreeMap::from([("flow.rs".into(), source.as_bytes().to_vec()), ("modules/extra.rs".into(), secondary.as_bytes().to_vec())]),
        BTreeMap::new(),
    ).unwrap()
}
fn with_dependency(id: &str, path: &str, dep: PackageSnapshot) -> PackageSnapshot {
    PackageSnapshot::capture(
        serde_json::to_string(&json!({"formatVersion":1,"id":id,"name":"Same Cargo name","entry":"flow.rs","files":["flow.rs"],"dependencies":{"dep":{"path":path}}})).unwrap(),
        BTreeMap::from([("flow.rs".into(), b"root entry".to_vec())]),
        BTreeMap::from([("dep".into(), dep)]),
    ).unwrap()
}
fn request(workspace: &Path, snapshot: PackageSnapshot, expected: Option<String>) -> PackageWrite {
    PackageWrite {
        workspace: workspace.into(),
        target: workspace
            .join(".zedflow/flow")
            .join(snapshot.root_manifest().unwrap().id.as_str()),
        snapshot,
        expected_revision: expected,
        publication: None,
        preconditions: vec![],
    }
}
fn marker(workspace: &Path) -> PathBuf {
    workspace.join(".zedflow/.package-acceptance.json")
}
fn read_marker(workspace: &Path) -> Value {
    serde_json::from_slice(&fs::read(marker(workspace)).unwrap()).unwrap()
}
fn edit_marker(workspace: &Path, value: &Value) {
    fs::write(marker(workspace), serde_json::to_vec(value).unwrap()).unwrap();
}
fn stage(workspace: &Path, value: &Value) -> PathBuf {
    workspace
        .join(".zedflow")
        .join(format!(".package-stage-{}", value["id"].as_str().unwrap()))
}

#[tokio::test]
async fn create_and_update_preserve_exact_manifest_and_secondary_files() {
    let workspace = tempfile::tempdir().unwrap();
    let original = snapshot("main", "first", "secondary");
    let write = request(workspace.path(), original.clone(), None);
    let target = write.target.clone();
    assert_eq!(
        flow_packages::begin(write)
            .await
            .unwrap()
            .finish()
            .await
            .unwrap(),
        original
    );
    let replacement = snapshot("main", "second", "secondary");
    let updated = flow_packages::begin(request(
        workspace.path(),
        replacement.clone(),
        Some(original.root),
    ))
    .await
    .unwrap()
    .finish()
    .await
    .unwrap();
    assert_eq!(updated, replacement);
    assert_eq!(
        fs::read(target.join("modules/extra.rs")).unwrap(),
        b"secondary"
    );
    assert_eq!(
        fs::read_to_string(target.join("flow.json")).unwrap(),
        replacement.root_node().unwrap().manifest_source
    );
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn collision_and_secondary_cas_conflicts_preserve_external_bytes() {
    let workspace = tempfile::tempdir().unwrap();
    let original = snapshot("main", "first", "secondary");
    let target = request(workspace.path(), original.clone(), None).target;
    flow_packages::begin(request(workspace.path(), original.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert!(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "new", "new"),
            None
        ))
        .await
        .is_err()
    );
    fs::write(target.join("modules/extra.rs"), "external").unwrap();
    assert!(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "new", "new"),
            Some(original.root)
        ))
        .await
        .is_err()
    );
    assert_eq!(
        fs::read(target.join("modules/extra.rs")).unwrap(),
        b"external"
    );
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn added_inventory_is_never_erased_by_save() {
    let workspace = tempfile::tempdir().unwrap();
    let original = snapshot("main", "first", "secondary");
    let target = request(workspace.path(), original.clone(), None).target;
    flow_packages::begin(request(workspace.path(), original.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    fs::write(target.join("extra.txt"), "external").unwrap();
    assert!(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "new", "new"),
            Some(original.root)
        ))
        .await
        .is_err()
    );
    assert_eq!(fs::read(target.join("extra.txt")).unwrap(), b"external");
}

#[tokio::test]
async fn nested_internal_dependencies_are_materialized_and_participate_in_cas() {
    let workspace = tempfile::tempdir().unwrap();
    let original = with_dependency(
        "main",
        "deps/child",
        with_dependency(
            "child",
            "deps/grandchild",
            snapshot("grandchild", "grandchild", "second"),
        ),
    );
    let target = request(workspace.path(), original.clone(), None).target;
    let actual = flow_packages::begin(request(workspace.path(), original.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(actual, original);
    let nested = target.join("deps/child/deps/grandchild/modules/extra.rs");
    assert_eq!(fs::read(&nested).unwrap(), b"second");
    fs::write(&nested, "external edit").unwrap();
    assert!(
        flow_packages::begin(request(
            workspace.path(),
            original.clone(),
            Some(original.root)
        ))
        .await
        .is_err()
    );
    assert_eq!(fs::read(nested).unwrap(), b"external edit");
}

#[tokio::test]
async fn external_dependency_is_frozen_but_never_written() {
    let workspace = tempfile::tempdir().unwrap();
    let dependency = snapshot("shared", "external", "second");
    flow_packages::begin(request(workspace.path(), dependency.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    let original = with_dependency("main", "../shared", dependency);
    flow_packages::begin(request(workspace.path(), original.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    let external = workspace.path().join(".zedflow/flow/shared/flow.rs");
    fs::write(&external, "changed").unwrap();
    assert!(
        flow_packages::begin(request(
            workspace.path(),
            original.clone(),
            Some(original.root)
        ))
        .await
        .is_err()
    );
    assert_eq!(fs::read(&external).unwrap(), b"changed");
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn sqlite_handoff_recovers_same_identity_and_payload_until_finish() {
    let workspace = tempfile::tempdir().unwrap();
    let mut write = request(workspace.path(), snapshot("main", "first", "second"), None);
    write.publication = Some(json!({"acceptedRevision":"host replay batch"}));
    let pending = flow_packages::begin(write).await.unwrap();
    let id = pending.id().to_owned();
    let payload = pending.publication().cloned();
    drop(pending);
    // Ordinary shared-lock readers cannot consume a SQL handoff.
    assert!(
        zf_storage::context_store::reader_lock(workspace.path().into())
            .await
            .is_err()
    );
    let recovered = flow_packages::recover(workspace.path().into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.id(), id);
    assert_eq!(recovered.publication(), payload.as_ref());
    drop(recovered);
    let recovered = flow_packages::recover(workspace.path().into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.id(), id);
    recovered.finish().await.unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn pure_save_is_completed_by_the_next_catalog_reader() {
    let workspace = tempfile::tempdir().unwrap();
    drop(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "first", "second"),
            None,
        ))
        .await
        .unwrap(),
    );
    assert!(marker(workspace.path()).exists());
    let guard = zf_storage::context_store::reader_lock(workspace.path().into())
        .await
        .unwrap();
    assert!(guard.is_some());
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn recovery_resumes_an_interrupted_partial_staging_tree() {
    let workspace = tempfile::tempdir().unwrap();
    let original = snapshot("main", "first", "secondary");
    let target = request(workspace.path(), original.clone(), None).target;
    drop(
        flow_packages::begin(request(workspace.path(), original.clone(), None))
            .await
            .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    fs::rename(&target, stage(workspace.path(), &intent)).unwrap();
    let fragment = workspace.path().join(".zedflow/.package-part-interrupted");
    fs::write(&fragment, "fi").unwrap();
    fs::remove_file(stage(workspace.path(), &intent).join("modules/extra.rs")).unwrap();
    intent["phase"] = json!("staging");
    edit_marker(workspace.path(), &intent);
    let recovered = flow_packages::recover(workspace.path().into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.finish().await.unwrap(), original);
    assert_eq!(fs::read(fragment).unwrap(), b"fi");
}

#[tokio::test]
async fn recovery_detects_creation_renamed_before_installed_marker() {
    let workspace = tempfile::tempdir().unwrap();
    drop(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "first", "second"),
            None,
        ))
        .await
        .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    intent["phase"] = json!("ready");
    edit_marker(workspace.path(), &intent);
    flow_packages::recover(workspace.path().into())
        .await
        .unwrap()
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn recovery_detects_exchange_before_installed_marker() {
    let workspace = tempfile::tempdir().unwrap();
    let old = snapshot("main", "old", "old second");
    flow_packages::begin(request(workspace.path(), old.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    let new = snapshot("main", "new", "new second");
    drop(
        flow_packages::begin(request(workspace.path(), new.clone(), Some(old.root)))
            .await
            .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    intent["phase"] = json!("ready");
    edit_marker(workspace.path(), &intent);
    assert_eq!(
        flow_packages::recover(workspace.path().into())
            .await
            .unwrap()
            .unwrap()
            .finish()
            .await
            .unwrap(),
        new
    );
    assert!(!stage(workspace.path(), &intent).exists());
}

#[tokio::test]
async fn finish_preserves_external_edits_after_installation() {
    let workspace = tempfile::tempdir().unwrap();
    let write = request(workspace.path(), snapshot("main", "first", "second"), None);
    let target = write.target.clone();
    let pending = flow_packages::begin(write).await.unwrap();
    fs::write(target.join("flow.rs"), "external").unwrap();
    assert!(pending.finish().await.is_err());
    assert_eq!(fs::read(target.join("flow.rs")).unwrap(), b"external");
    assert!(!marker(workspace.path()).exists());
}

#[tokio::test]
async fn recovery_preserves_external_backup_edits() {
    let workspace = tempfile::tempdir().unwrap();
    let old = snapshot("main", "old", "old second");
    flow_packages::begin(request(workspace.path(), old.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    drop(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "new", "new second"),
            Some(old.root),
        ))
        .await
        .unwrap(),
    );
    let intent = read_marker(workspace.path());
    let backup = stage(workspace.path(), &intent).join("flow.rs");
    fs::write(&backup, "external backup edit").unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .unwrap()
            .unwrap()
            .finish()
            .await
            .is_err()
    );
    assert_eq!(fs::read(backup).unwrap(), b"external backup edit");
    assert!(marker(workspace.path()).exists());
}

#[tokio::test]
async fn recovery_rejects_unvalidated_marker_paths() {
    let workspace = tempfile::tempdir().unwrap();
    drop(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "first", "second"),
            None,
        ))
        .await
        .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    intent["id"] = json!("../../outside");
    edit_marker(workspace.path(), &intent);
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .is_err()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_targets_and_staging_are_rejected_without_following() {
    use std::os::unix::fs::symlink;
    let workspace = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(workspace.path().join(".zedflow/flow")).unwrap();
    let write = request(workspace.path(), snapshot("main", "first", "second"), None);
    symlink(outside.path(), &write.target).unwrap();
    assert!(flow_packages::begin(write).await.is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    fs::remove_file(workspace.path().join(".zedflow/flow/main")).unwrap();
    drop(
        flow_packages::begin(request(
            workspace.path(),
            snapshot("main", "first", "second"),
            None,
        ))
        .await
        .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    intent["phase"] = json!("staging");
    edit_marker(workspace.path(), &intent);
    fs::rename(
        workspace.path().join(".zedflow/flow/main"),
        workspace.path().join("held"),
    )
    .unwrap();
    symlink(outside.path(), stage(workspace.path(), &intent)).unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .is_err()
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn recovery_before_creation_rejects_a_concurrent_target() {
    let workspace = tempfile::tempdir().unwrap();
    let write = request(workspace.path(), snapshot("main", "first", "second"), None);
    let target = write.target.clone();
    drop(flow_packages::begin(write).await.unwrap());
    let mut intent = read_marker(workspace.path());
    fs::rename(&target, stage(workspace.path(), &intent)).unwrap();
    intent["phase"] = json!("ready");
    edit_marker(workspace.path(), &intent);
    fs::create_dir(&target).unwrap();
    fs::write(target.join("external.txt"), "external creation").unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .is_err()
    );
    assert_eq!(
        fs::read(target.join("external.txt")).unwrap(),
        b"external creation"
    );
    assert!(marker(workspace.path()).exists());
}

#[tokio::test]
async fn recovery_rejects_external_changes_to_a_completed_staged_file() {
    let workspace = tempfile::tempdir().unwrap();
    let write = request(workspace.path(), snapshot("main", "first", "second"), None);
    let target = write.target.clone();
    drop(flow_packages::begin(write).await.unwrap());
    let mut intent = read_marker(workspace.path());
    let staged = stage(workspace.path(), &intent);
    fs::rename(&target, &staged).unwrap();
    intent["phase"] = json!("staging");
    edit_marker(workspace.path(), &intent);
    fs::write(staged.join("flow.rs"), "fi").unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .is_err()
    );
    assert_eq!(fs::read(staged.join("flow.rs")).unwrap(), b"fi");
    assert!(!target.exists());
}

#[tokio::test]
async fn recovery_resumes_cleanup_after_some_backup_files_were_removed() {
    let workspace = tempfile::tempdir().unwrap();
    let old = snapshot("main", "old", "old second");
    flow_packages::begin(request(workspace.path(), old.clone(), None))
        .await
        .unwrap()
        .finish()
        .await
        .unwrap();
    let new = snapshot("main", "new", "new second");
    drop(
        flow_packages::begin(request(workspace.path(), new.clone(), Some(old.root)))
            .await
            .unwrap(),
    );
    let mut intent = read_marker(workspace.path());
    intent["phase"] = json!("finishing");
    edit_marker(workspace.path(), &intent);
    fs::remove_file(stage(workspace.path(), &intent).join("modules/extra.rs")).unwrap();
    assert_eq!(
        flow_packages::recover(workspace.path().into())
            .await
            .unwrap()
            .unwrap()
            .finish()
            .await
            .unwrap(),
        new
    );
    assert!(!stage(workspace.path(), &intent).exists());
}

#[tokio::test]
async fn preconditions_are_revalidated_before_publication_recovery() {
    let workspace = tempfile::tempdir().unwrap();
    let dependency = workspace.path().join("definition.rs");
    fs::write(&dependency, "original").unwrap();
    let mut write = request(workspace.path(), snapshot("main", "first", "second"), None);
    write
        .preconditions
        .push(zf_storage::source_acceptance::FilePrecondition {
            path: dependency.clone(),
            hash: zf_storage::context_store::hash(b"original"),
        });
    let target = write.target.clone();
    drop(flow_packages::begin(write).await.unwrap());
    let mut intent = read_marker(workspace.path());
    fs::rename(&target, stage(workspace.path(), &intent)).unwrap();
    intent["phase"] = json!("ready");
    edit_marker(workspace.path(), &intent);
    fs::write(&dependency, "external").unwrap();
    assert!(
        flow_packages::recover(workspace.path().into())
            .await
            .is_err()
    );
    assert_eq!(fs::read(dependency).unwrap(), b"external");
    assert!(!target.exists());
}

#[tokio::test]
async fn cancellation_after_journaling_keeps_an_owned_worker_and_recoverable_result() {
    let workspace = tempfile::tempdir().unwrap();
    let expected = snapshot("main", &"x".repeat(1024 * 1024), "secondary");
    let write = request(workspace.path(), expected.clone(), None);
    let writer = tokio::spawn(flow_packages::begin(write));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !marker(workspace.path()).exists() {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    writer.abort();
    drop(writer.await);
    let recovered = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        flow_packages::recover(workspace.path().into()),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(recovered.finish().await.unwrap(), expected);
}
