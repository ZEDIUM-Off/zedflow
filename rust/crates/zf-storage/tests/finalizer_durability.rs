//! Cancellation must not release catalogue ownership while finalization is queued.
use std::{
    collections::BTreeMap,
    fs::File,
    future::{Future, poll_fn},
    path::Path,
    sync::mpsc,
    task::Poll,
};
use zf_flows::package::PackageSnapshot;
use zf_storage::{
    flow_packages::{self, PackageWrite},
    source_acceptance,
};

async fn cancel_queued_finalizer<F: Future>(workspace: &Path, finalizer: F, marker: &str) {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let blocker = tokio::task::spawn_blocking(move || {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    started_rx.await.unwrap();
    let mut finalizer = Box::pin(finalizer);
    let queued = poll_fn(|cx| Poll::Ready(finalizer.as_mut().poll(cx).is_pending())).await;
    drop(finalizer);
    let probe = File::open(workspace.join(".zedflow/.sources.lock")).unwrap();
    let lock_result = probe.try_lock();
    let retained = matches!(lock_result, Err(std::fs::TryLockError::WouldBlock));
    if lock_result.is_ok() {
        probe.unlock().unwrap();
    }
    // Always release the worker before assertions, including on the failing regression.
    release_tx.send(()).unwrap();
    blocker.await.unwrap();
    tokio::task::spawn_blocking(|| ()).await.unwrap();
    assert!(queued, "finalizer must queue behind the occupied worker");
    assert!(
        retained,
        "cancelled future released a finalizer's catalogue lock"
    );
    assert!(!workspace.join(".zedflow").join(marker).exists());
    probe.try_lock().unwrap();
    probe.unlock().unwrap();
}

#[test]
fn package_finalizer_keeps_lock_after_caller_cancellation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    runtime.block_on(async {
        let snapshot = PackageSnapshot::capture(
            r#"{"formatVersion":1,"id":"sample","name":"Sample","entry":"flow.rs","files":["flow.rs"]}"#.into(),
            BTreeMap::from([("flow.rs".into(), b"// captured".to_vec())]), BTreeMap::new()
        ).unwrap();
        let pending = flow_packages::begin(PackageWrite {
            workspace: workspace.path().into(), target: workspace.path().join(".zedflow/flow/sample"),
            snapshot, expected_revision: None, publication: None, preconditions: vec![],
        }).await.unwrap();
        cancel_queued_finalizer(workspace.path(), pending.finish(), ".package-acceptance.json").await;
    });
}

#[test]
fn source_finalizer_keeps_lock_after_caller_cancellation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    runtime.block_on(async {
        let pending = source_acceptance::begin_flow(
            workspace.path().join(".zedflow/flows/sample.rs"),
            workspace.path().into(),
            "// captured".into(),
            None,
            None,
            vec![],
        )
        .await
        .unwrap();
        cancel_queued_finalizer(
            workspace.path(),
            pending.finish(),
            ".source-acceptance.json",
        )
        .await;
    });
}
