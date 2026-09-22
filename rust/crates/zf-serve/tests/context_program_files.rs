use serde_json::json;
use std::collections::BTreeMap;
use zf_compiler::programs;
use zf_context::context::*;
use zf_core::types::DataType;
use zf_flows::schema::Composition;
use zf_storage::context_store::ContextStore;
use zf_storage::context_store::LibraryStore;
fn doc() -> Composition {
    serde_json::from_value(json!({"formatVersion":2,"id":"test","name":"Context","nodes":[
{"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
{"id":"agent","position":{"x":0,"y":0},"data":{"kind":"agent","label":"Agent","config":{"provider":"fixture","fixtureSteps":[{"echoRequest":true}],"contextStrategy":{"key":"test"},"contextBindings":{"input":{"kind":"state","field":"input"}},"contextLibraryRef":{"key":"shared"}}}},
{"id":"end","position":{"x":0,"y":0},"data":{"kind":"end","label":"End","config":{}}}
],"edges":[{"id":"a","source":"start","target":"agent"},{"id":"b","source":"agent","target":"end"}]})).unwrap()
}
fn strategy() -> ContextStrategy {
    ContextStrategy::new("test", "Test")
        .require("input", DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "prompt",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::Call {
                catalog: LibraryKind::Projection,
                name: "display".into(),
                arguments: BTreeMap::from([("text".into(), ContextExpr::resource("input"))]),
            },
        )])
}
#[tokio::test]
async fn exact_file_sources_are_frozen_and_survive_catalog_changes() {
    let root = tempfile::tempdir().unwrap();
    let store = ContextStore::new(root.path().into());
    let saved = store.save(&strategy(), None).await.unwrap();
    let libraries = LibraryStore::new(root.path().into());
    let library = ContextLibrary::new().projection(
        "display",
        ContextFunction::new(
            BTreeMap::from([("text".into(), DataType::Text)]),
            DataType::Text,
            ContextExpr::Variable {
                name: "text".into(),
            },
        ),
    );
    let lib = libraries.save("shared", &library, None).await.unwrap();
    let mut document = doc();
    let dependencies = async {
        let sources = zf_execution::sources::program_sources(&document, root.path(), &[]).await?;
        programs::freeze(&mut document, &sources)
    }
    .await
    .unwrap();
    assert_eq!(dependencies[0].hash, saved.hash);
    let frozen = document.nodes[1].data.config["contextProgram"].clone();
    assert_eq!(frozen["source"], saved.source.unwrap());
    assert_eq!(frozen["librarySources"][0]["hash"], lib.hash);
    std::fs::write(lib.path, "external invalid edit").unwrap();
    std::fs::write(saved.path, "external invalid edit").unwrap();
    programs::validate_frozen(&frozen).unwrap();
    zf_compiler::graph_compiler::validate(&document, &zf_runtime::materialize::RuntimePrimitives)
        .unwrap();
    let mut new_run = doc();
    assert!(
        async {
            let sources =
                zf_execution::sources::program_sources(&new_run, root.path(), &[]).await?;
            programs::freeze(&mut new_run, &sources)
        }
        .await
        .is_err()
    );
    let mut tampered = frozen;
    tampered["library"]["projections"]["display"]["body"] =
        json!({"kind":"resource","name":"input"});
    assert!(programs::validate_frozen(&tampered).is_err());
}
#[tokio::test]
async fn missing_file_binding_is_a_compile_diagnostic_not_an_implicit_source() {
    let root = tempfile::tempdir().unwrap();
    let store = ContextStore::new(root.path().into());
    let simple = ContextStrategy::new("test", "Test")
        .require("input", DataType::Text)
        .with_program(vec![]);
    store.save(&simple, None).await.unwrap();
    let mut document = doc();
    let cfg = document.nodes[1].data.config.as_object_mut().unwrap();
    cfg.remove("contextLibraryRef");
    cfg.insert("contextBindings".into(), json!({}));
    let error = async {
        let sources = zf_execution::sources::program_sources(&document, root.path(), &[]).await?;
        programs::freeze(&mut document, &sources)
    }
    .await
    .unwrap_err();
    assert!(error.to_string().contains("sans binding"));
}
