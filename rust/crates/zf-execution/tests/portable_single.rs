use adk_graph::State;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use zf_compiler::graph_compiler::GraphValidator;
use zf_execution::{
    route_runtime::NativeFactory,
    runtime_export::{RunOptions, run_single},
};
use zf_flows::{flow_source, package::PackageSnapshot, schema::Composition};
use zf_runtime::{
    materialize::RuntimePrimitives, revisions::RevisionDefinition,
    workspace_context::ContextSnapshot,
};

fn definition() -> RevisionDefinition {
    let node = |id: &str, kind: &str, config: Value| json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}});
    let edge = |a: &str, b: &str| json!({"id":format!("{a}-{b}"),"source":a,"target":b});
    let composition:Composition = serde_json::from_value(json!({"formatVersion":3,"id":"portable","name":"Portable",
        "nodes":[node("start","start",json!({})),node("effect","tool",json!({"tool":"exec","arguments":{"command":"printf x >> effects"}})),
        node("pause","input",json!({"field":"output","prompt":"Continue?","responseType":"text"})),node("publish","output",json!({"inputField":"output"})),node("end","end",json!({}))],
        "edges":[edge("start","effect"),edge("effect","pause"),edge("pause","publish"),edge("publish","end")]})).unwrap();
    let source =
        flow_source::render(&composition, &GraphValidator::new(&RuntimePrimitives)).unwrap();
    RevisionDefinition {
        key: "portable".into(),
        hash: format!("{:x}", Sha256::digest(source.as_bytes())),
        source,
        composition,
        package: None,
        context_selections: BTreeMap::new(),
    }
}
fn package(source: &str, readme: &str) -> PackageSnapshot {
    PackageSnapshot::capture(json!({"formatVersion":1,"id":"portable","name":"Portable","entry":"flow.rs","files":["flow.rs","README.md"]}).to_string(),
        BTreeMap::from([("flow.rs".into(),source.as_bytes().to_vec()),("README.md".into(),readme.as_bytes().to_vec())]),BTreeMap::new()).unwrap()
}
fn options(root: &Path, input: Value) -> RunOptions {
    RunOptions {
        workspace: root.join("workspace"),
        data: root.join("data"),
        run_id: "portable".into(),
        input: serde_json::from_value::<State>(input).unwrap(),
        models: None,
        capabilities: None,
    }
}
fn seed(root: &Path) {
    std::fs::create_dir_all(root.join("workspace")).unwrap();
    std::fs::create_dir_all(root.join("data/portable")).unwrap();
    std::fs::write(
        root.join("data/portable/context.json"),
        serde_json::to_vec(&ContextSnapshot {
            cwd: root.join("workspace"),
            ..Default::default()
        })
        .unwrap(),
    )
    .unwrap();
}
fn factory(def: &RevisionDefinition, calls: &Arc<AtomicUsize>) -> NativeFactory {
    let doc = def.composition.clone();
    let calls = calls.clone();
    Arc::new(move |services, checkpoint, scope| {
        assert_eq!(scope, "");
        calls.fetch_add(1, Ordering::SeqCst);
        zf_runtime::materialize::build_scope(&doc, None, scope, Some(services), Some(checkpoint))
    })
}
#[tokio::test]
async fn standalone_resumes_without_public_ports_or_repeating_recorded_effects() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let def = definition();
    assert!(
        zf_flows::flow_contract::read(&def.composition)
            .unwrap()
            .is_none()
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let native = factory(&def, &calls);
    let first = run_single(
        def.clone(),
        native.clone(),
        options(temp.path(), json!({"input":"begin"})),
    )
    .await
    .unwrap();
    assert_eq!(first["status"], "waiting", "{first}");
    let second = run_single(
        def,
        native,
        options(temp.path(), json!({"answer:pause":"réponse 🦀"})),
    )
    .await
    .unwrap();
    assert_eq!(second["status"], "completed", "{second}");
    assert_eq!(second["state"]["response"], "réponse 🦀");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/effects")).unwrap(),
        "x"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let events = std::fs::read_to_string(temp.path().join("data/portable/events.jsonl")).unwrap();
    assert!(events.contains("receiptRef"));
    assert!(events.contains("checkpoint"));
    zf_storage::migration::lock(&temp.path().join("data")).unwrap();
}
#[tokio::test]
async fn exported_run_rejects_changed_source_package_or_workspace_before_effects() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let mut def = definition();
    def.package = Some(package(&def.source, "original"));
    let calls = Arc::new(AtomicUsize::new(0));
    let native = factory(&def, &calls);
    run_single(def.clone(), native.clone(), options(temp.path(), json!({})))
        .await
        .unwrap();
    let mut corrupt = def.clone();
    corrupt.source.push('\n');
    assert!(
        run_single(corrupt, native.clone(), options(temp.path(), json!({})))
            .await
            .is_err()
    );
    let mut changed = def.clone();
    changed.package = Some(package(&def.source, "new file"));
    let error = run_single(changed, native.clone(), options(temp.path(), json!({})))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("different workspace or frozen runtime"),
        "{error:#}"
    );
    let mut other = options(temp.path(), json!({}));
    other.workspace = temp.path().join("other");
    std::fs::create_dir(&other.workspace).unwrap();
    assert!(run_single(def, native, other).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/effects")).unwrap(),
        "x"
    );
}
#[tokio::test]
async fn invalid_ids_and_an_existing_data_owner_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let def = definition();
    let calls = Arc::new(AtomicUsize::new(0));
    for id in ["", "..", "../outside", "a/b", "a\\b", "a\0b"] {
        let mut args = options(temp.path(), json!({}));
        args.run_id = id.into();
        assert!(
            run_single(def.clone(), factory(&def, &calls), args)
                .await
                .unwrap_err()
                .to_string()
                .contains("directory-safe")
        );
    }
    let _lease = zf_storage::migration::lock(&temp.path().join("data")).unwrap();
    assert!(
        run_single(
            def.clone(),
            factory(&def, &calls),
            options(temp.path(), json!({}))
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("utilisé")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!temp.path().join("data/sessions.db").exists());
}
#[tokio::test]
async fn native_build_failure_drains_the_log_and_releases_ownership() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        run_single(
            definition(),
            Arc::new(|_, _, _| anyhow::bail!("native fixture failure")),
            options(temp.path(), json!({})),
        ),
    )
    .await
    .unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("native fixture failure")
    );
    zf_storage::migration::lock(&temp.path().join("data")).unwrap();
    assert!(!temp.path().join("workspace/effects").exists());
}

#[tokio::test]
async fn native_panic_keeps_cleanup_inside_the_data_ownership_boundary() {
    let temp = tempfile::tempdir().unwrap();
    seed(temp.path());
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        run_single(
            definition(),
            Arc::new(|_, _, _| panic!("fixture native panic")),
            options(temp.path(), json!({})),
        ),
    )
    .await
    .unwrap();
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("native export execution panicked")
    );
    let lease = zf_storage::migration::lock(&temp.path().join("data")).unwrap();
    assert!(!temp.path().join("workspace/effects").exists());
    drop(lease);
    let def = definition();
    let calls = Arc::new(AtomicUsize::new(0));
    let resumed = run_single(
        def.clone(),
        factory(&def, &calls),
        options(temp.path(), json!({})),
    )
    .await
    .unwrap();
    assert_eq!(resumed["status"], "waiting");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("workspace/effects")).unwrap(),
        "x"
    );
}
