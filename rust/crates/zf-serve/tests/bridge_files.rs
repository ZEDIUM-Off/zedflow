use serde_json::json;
use zf_flows::bridge_source;
use zf_flows::composition::*;
use zf_storage::bridge_store::BridgeStore;
fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        .require("base")
        .import("reviewer", "review-flow")
        .reuse("context", "context-flow", "base/context")
        .connect(
            "review",
            Connection::new(
                Endpoint::new("root", "review"),
                Endpoint::new("reviewer", "start"),
                RouteMode::CallAwait,
                InvocationKind::Tool,
            )
            .tool("review_docs"),
        )
        .connect(
            "background",
            Connection::new(
                Endpoint::new("root", "index"),
                Endpoint::new("context", "start"),
                RouteMode::Launch,
                InvocationKind::Condition,
            )
            .when(json!({"kind":"compare","operator":"eq","field":"changed","value":true})),
        )
        .connect(
            "transfer",
            Connection::new(
                Endpoint::new("root", "phase"),
                Endpoint::new("reviewer", "start"),
                RouteMode::Handoff,
                InvocationKind::Node,
            ),
        )
        .bind(
            "context",
            Endpoint::new("context", "terms"),
            Endpoint::new("root", "terms"),
            DataPermissions::read_only(),
        )
}
#[test]
fn structured_source_round_trips_every_route_and_explicit_reuse() {
    let definition = bridge();
    let source = bridge_source::generate(&definition).unwrap();
    assert!(source.contains(".connect(\"review\""));
    assert!(!source.contains("from_str"));
    assert_eq!(
        serde_json::to_value(bridge_source::parse(&source).unwrap()).unwrap(),
        serde_json::to_value(definition).unwrap()
    );
    assert!(
        bridge_source::parse(
            &source.replace(".require(\"base\")", ".require(\"base\").require(\"base\")")
        )
        .is_err()
    );
    assert!(
        bridge_source::parse(&source.replace("pub fn bridge()", "#[test]\npub fn bridge()"))
            .is_err()
    );
    assert!(bridge_source::parse(&(source + "\nfn execute_hidden() {}\n")).is_err());
}
#[tokio::test]
async fn file_catalog_detects_conflicts_external_edits_and_invalid_sources() {
    let root = tempfile::tempdir().unwrap();
    let store = BridgeStore::new(root.path().into()).unwrap();
    let saved = store.save("documentation", &bridge(), None).await.unwrap();
    assert!(saved.path.ends_with(".zedflow/bridges/documentation.rs"));
    assert!(store.save("documentation", &bridge(), None).await.is_err());
    std::fs::write(&saved.path, "// invalid\nfn arbitrary(){}\n").unwrap();
    assert!(
        store
            .save("documentation", &bridge(), Some(&saved.hash))
            .await
            .is_err()
    );
    let listed = store.list().await.unwrap();
    assert!(listed[0].bridge.is_none());
    assert!(!listed[0].diagnostics.is_empty());
    assert!(store.catalog().await.unwrap().is_empty());
    assert!(store.save("../escape", &bridge(), None).await.is_err());
}
#[cfg(unix)]
#[tokio::test]
async fn bridge_file_never_follows_a_symlink() {
    let root = tempfile::tempdir().unwrap();
    let store = BridgeStore::new(root.path().into()).unwrap();
    let saved = store.save("safe", &bridge(), None).await.unwrap();
    let target = root.path().join("outside.rs");
    std::fs::write(&target, "preserve").unwrap();
    std::fs::remove_file(&saved.path).unwrap();
    std::os::unix::fs::symlink(&target, &saved.path).unwrap();
    assert!(
        store
            .save("safe", &bridge(), Some(&saved.hash))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(target).unwrap(), "preserve");
}
