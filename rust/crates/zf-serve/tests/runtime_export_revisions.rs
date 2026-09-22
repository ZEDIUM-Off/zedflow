use adk_graph::State;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use zf_storage::content_store::ContentStore;

use zf_compiler::prepared::FrozenFlow;
use zf_compiler::prepared::PreparedRuntime;
use zf_execution::route_runtime::NativeFactory;
use zf_execution::runtime_export;
use zf_execution::runtime_export::RunOptions;
use zf_flows::composition::CompositionCatalog;
use zf_flows::composition::ResolveRequest;
use zf_flows::flow_contract;
use zf_flows::schema::Composition;
use zf_runtime::revisions::Compatibility;
use zf_runtime::revisions::RevisionDefinition;
use zf_runtime::revisions::RevisionRuntime;
fn doc() -> Composition {
    serde_json::from_value(json!({"formatVersion":3,"id":"portable-root","name":"Portable root","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{"exports":{"contract":{"entries":{"main":{"input":{"kind":"text"},"output":{"kind":"text"}}}},"entries":{"main":{"node":"start","inputField":"input","outputField":"response"}},"interactive":true}}}},
        {"id":"gate","position":{"x":0,"y":100},"data":{"kind":"set","label":"Gate","config":{"field":"output","value":"old"}}},
        {"id":"pause","position":{"x":0,"y":150},"data":{"kind":"input","label":"Pause","config":{"field":"output","prompt":"Question","responseType":"text"}}},
        {"id":"publish","position":{"x":0,"y":200},"data":{"kind":"output","label":"Publish","config":{"inputField":"output"}}},
        {"id":"end","position":{"x":0,"y":300},"data":{"kind":"end","label":"End","config":{}}}
    ],"edges":[{"id":"a","source":"start","target":"gate"},{"id":"b","source":"gate","target":"pause"},{"id":"p","source":"pause","target":"publish"},{"id":"c","source":"publish","target":"end"}]})).unwrap()
}
fn definition(composition: Composition) -> RevisionDefinition {
    let source = zf_flows::flow_source::render(
        &composition,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )
    .unwrap();
    RevisionDefinition {
        package: None,
        context_selections: Default::default(),
        key: composition.id.clone(),
        hash: format!("{:x}", Sha256::digest(source.as_bytes())),
        source,
        composition,
    }
}
#[tokio::test]
async fn portable_root_consumes_structural_boundaries_and_resumes_the_checkpoint_definition() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let data = temp.path().join("data");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&data).unwrap();
    let old = definition(doc());
    let exports = flow_contract::read(&old.composition).unwrap().unwrap();
    let catalog = CompositionCatalog {
        flows: BTreeMap::from([(old.key.clone(), exports.contract.clone())]),
        ..Default::default()
    };
    let graph = zf_compiler::resolve::resolve(
        &catalog,
        &ResolveRequest {
            flow: old.key.clone(),
            entry: "main".into(),
            bridges: vec![],
        },
    )
    .unwrap();
    let prepared = PreparedRuntime {
        graph,
        flows: BTreeMap::from([(
            "root".into(),
            FrozenFlow {
                key: old.key.clone(),
                hash: old.hash.clone(),
                source: old.source.clone(),
                composition: old.composition.clone(),
                exports,
            },
        )]),
        definitions: Default::default(),
    };
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(data.join("sessions.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let content = ContentStore::new(pool).await.unwrap();
    let revisions = RevisionRuntime::new(
        content,
        "export-resume",
        BTreeMap::from([("root".into(), old.clone())]),
    )
    .await
    .unwrap();
    let mut updated = old.composition.clone();
    updated.nodes.push(serde_json::from_value(json!({"id":"new_tail","position":{"x":0,"y":170},"data":{"kind":"set","label":"New tail","config":{"field":"output","value":"adopted {{output}}"}}})).unwrap());
    updated
        .edges
        .iter_mut()
        .find(|edge| edge.source == "pause")
        .unwrap()
        .target = "new_tail".into();
    updated.edges.push(
        serde_json::from_value(json!({"id":"added","source":"new_tail","target":"publish"}))
            .unwrap(),
    );
    assert!(matches!(
        revisions
            .publish("root", definition(updated))
            .await
            .unwrap(),
        Compatibility::SequentialBoundary { .. }
    ));
    let called = Arc::new(AtomicUsize::new(0));
    let factory: NativeFactory = {
        let called = called.clone();
        let doc = old.composition.clone();
        Arc::new(move |_, _, _| {
            called.fetch_add(1, Ordering::SeqCst);
            zf_runtime::materialize::build(&doc)
        })
    };
    let options = |input: Value| RunOptions {
        workspace: workspace.clone(),
        home: Some(workspace.join(".fixture-home")),
        data: data.clone(),
        run_id: "export-resume".into(),
        input: serde_json::from_value::<State>(input).unwrap(),
        models: None,
        capabilities: None,
    };
    let waiting = runtime_export::run(
        prepared.clone(),
        BTreeMap::from([("root".into(), factory.clone())]),
        options(json!({"input":"begin"})),
    )
    .await
    .unwrap();
    assert_eq!(waiting["status"], "waiting", "{waiting}");
    assert!(
        waiting["interrupt"].to_string().contains("root/pause"),
        "{waiting}"
    );
    assert!(
        !waiting["interrupt"]
            .to_string()
            .contains("revision_boundary"),
        "{waiting}"
    );
    assert_eq!(called.load(Ordering::SeqCst), 1);
    let resumed = runtime_export::run(
        prepared,
        BTreeMap::from([("root".into(), factory)]),
        options(json!({"answer:pause":"from adopted source"})),
    )
    .await
    .unwrap();
    assert_eq!(resumed["status"], "completed", "{resumed}");
    assert_eq!(resumed["state"]["response"], "adopted from adopted source");
    assert_eq!(
        called.load(Ordering::SeqCst),
        1,
        "initial native source must not reconstruct an adopted structure"
    );
    let events = std::fs::read_to_string(data.join("export-resume/events.jsonl")).unwrap();
    assert!(events.contains("revision_adopted"));
}
