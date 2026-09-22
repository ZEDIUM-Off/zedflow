use serde_json::json;
use std::collections::BTreeMap;
use zf_context::context::ContextBlock as B;
use zf_context::context::ContextExpr as E;
use zf_context::context::ContextFunction;
use zf_context::context::ContextLibrary;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat as F;
use zf_context::context::FragmentRole as R;
use zf_context::context::LibraryKind;
use zf_context::context_package;
use zf_context::context_package::ArtifactKind as K;
use zf_context::context_package::ArtifactSelection;
use zf_context::context_package::ContextPackage;
use zf_context::context_package::SourceArtifact;
use zf_context::context_source::generate;
use zf_context::context_source::generate_library;
use zf_context::context_source::generate_types;
use zf_context::context_source::parse_types;
use zf_core::types::DataType as T;
use zf_core::types::TypeRegistry;
use zf_flows::bridge_source;
use zf_flows::composition::BridgeDefinition;
use zf_storage::context_store;
use zf_storage::context_store::SourceStore;
fn artifact(kind: K, key: &str, source: String) -> SourceArtifact {
    SourceArtifact {
        kind,
        key: key.into(),
        hash: context_store::hash(source.as_bytes()),
        source,
    }
}

#[tokio::test]
async fn json_examples_share_exact_sources_and_import_with_their_definitions_atomically() {
    let origin = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let example = zf_storage::source_catalog::examples::save(
        origin.path().into(),
        T::Text,
        TypeRegistry::new(),
        "Été 🦀".into(),
        json!("Texte partagé"),
    )
    .await
    .unwrap();
    let mut package = zf_storage::context_store::packages::export_selection(
        origin.path().into(),
        &[ArtifactSelection {
            kind: K::Example,
            key: example.id.clone(),
        }],
    )
    .await
    .unwrap();
    assert_eq!(package.version, 2);
    let source = package.artifacts[0].source.clone();
    package.artifacts.extend(fixture().artifacts);
    assert!(
        context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .valid
    );
    zf_storage::context_store::packages::import_package(target.path().into(), &package)
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(
            target
                .path()
                .join(format!(".zedflow/examples/{}.json", example.id))
        )
        .unwrap(),
        source
    );
    assert!(
        !target
            .path()
            .join(format!(".zedflow/examples/{}.rs", example.id))
            .exists()
    );
    assert!(
        zf_storage::source_catalog::examples::list(
            target.path().into(),
            T::Text,
            TypeRegistry::new()
        )
        .await
        .unwrap()
        .iter()
        .any(|item| item.id == example.id)
    );
    let collision = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(collision.path().join(".zedflow/examples")).unwrap();
    std::fs::write(
        collision
            .path()
            .join(format!(".zedflow/examples/{}.json", example.id)),
        &source,
    )
    .unwrap();
    assert!(
        zf_storage::context_store::packages::import_package(collision.path().into(), &package)
            .await
            .is_err()
    );
    assert!(!collision.path().join(".zedflow/context/review.rs").exists());
    let mut value: serde_json::Value = serde_json::from_str(&package.artifacts[0].source).unwrap();
    value["schemaHash"] = json!("wrong");
    package.artifacts[0] = artifact(K::Example, &example.id, value.to_string());
    assert!(
        !context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .valid
    );
}
fn fixture() -> ContextPackage {
    let named = T::Named {
        name: "Report".into(),
    };
    let registry = TypeRegistry::from([(
        "Report".into(),
        T::Record {
            fields: BTreeMap::from([("title".into(), T::Text)]),
        },
    )]);
    let library = ContextLibrary::new().projection(
        "title",
        ContextFunction::new(
            BTreeMap::from([("report".into(), named.clone())]),
            T::Text,
            E::field(E::variable("report"), "title"),
        ),
    );
    let strategy = ContextStrategy::new("review", "Review")
        .require("report", named)
        .with_program(vec![B::emit(
            "title",
            R::Data,
            F::Text,
            E::call(
                LibraryKind::Projection,
                "title",
                BTreeMap::from([("report".into(), E::resource("report"))]),
            ),
        )]);
    let bridge = BridgeDefinition::new().import("worker", "external-research-flow");
    ContextPackage {
        version: 1,
        artifacts: vec![
            artifact(K::Strategy, "review", generate(&strategy).unwrap()),
            artifact(K::Library, "reports", generate_library(&library).unwrap()),
            artifact(
                K::Bridge,
                "research",
                bridge_source::generate(&bridge).unwrap(),
            ),
            artifact(K::Types, "domain", generate_types(&registry).unwrap()),
        ],
    }
}
#[tokio::test]
async fn export_import_preserves_exact_rust_sources_and_reports_external_flow_prerequisites() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut package = fixture();
    package.artifacts[0]
        .source
        .push_str("\n// Author comment remains exact.\n");
    package.artifacts[0].hash = context_store::hash(package.artifacts[0].source.as_bytes());
    let validation = context_package::validate_package(
        &package,
        &zf_flows::bridge_source::PackageBridgeValidator,
    );
    assert!(validation.valid, "{:?}", validation.diagnostics);
    assert_eq!(validation.prerequisites[0].kind, "flow");
    assert_eq!(validation.prerequisites[0].key, "external-research-flow");
    let imported =
        zf_storage::context_store::packages::import_package(first.path().into(), &package)
            .await
            .unwrap();
    assert_eq!(imported.files.len(), 4);
    assert!(!first.path().join(".zedflow/flows").exists());
    let selection: Vec<_> = package
        .artifacts
        .iter()
        .map(|a| ArtifactSelection {
            kind: a.kind,
            key: a.key.clone(),
        })
        .collect();
    let exported =
        zf_storage::context_store::packages::export_selection(first.path().into(), &selection)
            .await
            .unwrap();
    assert_eq!(exported, package);
    zf_storage::context_store::packages::import_package(second.path().into(), &exported)
        .await
        .unwrap();
    for (a, b) in imported.files.iter().zip(
        zf_storage::context_store::packages::export_selection(second.path().into(), &selection)
            .await
            .unwrap()
            .artifacts,
    ) {
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.source.as_deref(), Some(b.source.as_str()));
    }
    assert!(
        parse_types(&package.artifacts[3].source)
            .unwrap()
            .contains_key("Report")
    );
}
#[tokio::test]
async fn collisions_and_invalid_packages_leave_all_target_sources_intact() {
    let workspace = tempfile::tempdir().unwrap();
    let store = SourceStore::new(workspace.path().into(), &["context", "libraries"]).unwrap();
    let original = store
        .save("reports", "// existing user source", None)
        .await
        .unwrap();
    assert!(
        zf_storage::context_store::packages::import_package(workspace.path().into(), &fixture())
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(original.path).unwrap(),
        "// existing user source"
    );
    assert!(!workspace.path().join(".zedflow/context/review.rs").exists());
    assert!(
        !workspace
            .path()
            .join(".zedflow/.source-import.json")
            .exists()
    );
    for mutate in [0, 1, 2, 3] {
        let empty = tempfile::tempdir().unwrap();
        let mut package = fixture();
        match mutate {
            0 => package.artifacts[0].hash = "wrong".into(),
            1 => package.artifacts[0].key = "../escape".into(),
            2 => package.artifacts.push(package.artifacts[0].clone()),
            _ => {
                package.artifacts[0].source = "fn execute() { panic!(); }".into();
                package.artifacts[0].hash =
                    context_store::hash(package.artifacts[0].source.as_bytes());
            }
        }
        assert!(
            zf_storage::context_store::packages::import_package(empty.path().into(), &package)
                .await
                .is_err()
        );
        assert!(!empty.path().join(".zedflow").exists());
    }
}
#[test]
fn package_validation_refuses_missing_types_conflicting_definitions_and_bridge_cycles() {
    let mut package = fixture();
    package.artifacts.retain(|a| a.kind != K::Types);
    assert!(
        !context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .valid
    );
    let mut package = fixture();
    let alternate = TypeRegistry::from([("Report".into(), T::Boolean)]);
    package.artifacts.push(artifact(
        K::Types,
        "conflict",
        generate_types(&alternate).unwrap(),
    ));
    assert!(
        context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .diagnostics
        .iter()
        .any(|d| d.code == "package_type_conflict")
    );
    let mut package = fixture();
    package.artifacts[2] = artifact(
        K::Bridge,
        "research",
        bridge_source::generate(&BridgeDefinition::new().require("research")).unwrap(),
    );
    assert!(
        context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .diagnostics
        .iter()
        .any(|d| d.code == "package_bridge_cycle")
    );
    let mut package = fixture();
    package.version = 99;
    assert!(
        !context_package::validate_package(
            &package,
            &zf_flows::bridge_source::PackageBridgeValidator
        )
        .valid
    );
    let serialized = serde_json::to_value(fixture()).unwrap();
    assert_eq!(serialized["version"], json!(1));
}
