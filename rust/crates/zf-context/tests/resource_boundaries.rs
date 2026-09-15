use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::{
    self, ContextBlock, ContextExpr, ContextPredicate, ContextStrategy, FragmentFormat,
    FragmentRole,
};
use zf_core::types::{DataType, TypeRegistry};

#[test]
fn only_live_branches_demand_resources_without_services() {
    let strategy = ContextStrategy::new_v2("branch", "Branch")
        .require("optional", DataType::Text)
        .require("selected", DataType::Text)
        .with_program(vec![ContextBlock::branch(
            "choice",
            ContextPredicate::present(ContextExpr::resource("optional")),
            vec![ContextBlock::emit(
                "out",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::resource("selected"),
            )],
            vec![],
        )]);
    let absent = context::evaluate(&strategy, &BTreeMap::new(), &TypeRegistry::new());
    assert!(absent.complete, "{:?}", absent.diagnostics);
    assert!(absent.needs.is_empty());
    assert_eq!(absent.reads, ["optional"]);
    assert_eq!(absent.trace[0].outcome, Some(false));
    let resources = BTreeMap::from([("optional".into(), Arc::new(json!("present")))]);
    let demanded = context::evaluate(&strategy, &resources, &TypeRegistry::new());
    assert!(!demanded.complete);
    assert_eq!(demanded.needs[0].resource, "selected");
    assert_eq!(
        demanded
            .trace
            .iter()
            .find(|trace| trace.block_id == "choice")
            .unwrap()
            .outcome,
        Some(true)
    );
    assert_eq!(*resources["optional"], json!("present"));
}

#[test]
fn windows_capture_absence_and_reject_invalid_patch_without_changing_input() {
    use zf_context::window::{self, WindowPatch};
    let strategy = ContextStrategy::new_v2("window", "Window")
        .require("optional", DataType::Text)
        .with_program(vec![ContextBlock::branch(
            "choice",
            ContextPredicate::present(ContextExpr::resource("optional")),
            vec![],
            vec![ContextBlock::emit(
                "fallback",
                FragmentRole::Data,
                FragmentFormat::Text,
                ContextExpr::literal(DataType::Text, json!("fallback")),
            )],
        )]);
    let evaluation = context::evaluate(&strategy, &BTreeMap::new(), &TypeRegistry::new());
    assert!(window::capture(&strategy, "s1", &evaluation, BTreeMap::new()).is_err());
    let captured = window::capture(
        &strategy,
        "s1",
        &evaluation,
        BTreeMap::from([("optional".into(), "absent".into())]),
    )
    .unwrap();
    let before = captured.clone();
    let patches = [
        WindowPatch::Representation {
            id: "fallback".into(),
            format: FragmentFormat::Text,
            value: json!("new"),
        },
        WindowPatch::Remove {
            id: "unknown".into(),
        },
    ];
    assert!(window::patched(&captured, &patches).is_err());
    assert_eq!(captured, before);
    assert_eq!(
        window::decode(&serde_json::to_value(&captured).unwrap()).unwrap(),
        captured
    );
}

#[test]
fn readers_require_explicit_installation_and_enforce_output_contracts() {
    use zf_context::resource_readers::{
        ReadResource, ReaderContract, ReaderOutput, ReaderRegistry, ResourceReader,
    };
    struct Reader;
    impl ResourceReader for Reader {
        fn contract(&self) -> ReaderContract {
            ReaderContract {
                id: "fixture".into(),
                version: "1".into(),
                input: DataType::Text,
                output: ReaderOutput::DeclaredJson,
            }
        }
        fn read<'a>(
            &'a self,
            input: &'a serde_json::Value,
        ) -> futures::future::BoxFuture<'a, anyhow::Result<Option<ReadResource>>> {
            Box::pin(async move {
                if input == "missing" {
                    return Ok(None);
                }
                Ok(Some(ReadResource {
                    value: Arc::new(json!(42)),
                    provenance: json!({"source":"fixture","extra":{"preserved":true}}),
                }))
            })
        }
    }
    futures::executor::block_on(async {
        let mut readers = ReaderRegistry::new();
        assert!(
            readers
                .read(
                    "fixture",
                    &json!("ok"),
                    &DataType::Number,
                    &TypeRegistry::new()
                )
                .await
                .is_err()
        );
        readers.register(Arc::new(Reader)).unwrap();
        assert!(readers.register(Arc::new(Reader)).is_err());
        assert!(
            readers
                .read(
                    "fixture",
                    &json!("missing"),
                    &DataType::Number,
                    &TypeRegistry::new()
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            readers
                .read(
                    "fixture",
                    &json!("ok"),
                    &DataType::Text,
                    &TypeRegistry::new()
                )
                .await
                .is_err()
        );
        let value = readers
            .read(
                "fixture",
                &json!("ok"),
                &DataType::Number,
                &TypeRegistry::new(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*value.value, json!(42));
        assert_eq!(value.provenance["source"]["extra"]["preserved"], true);
    });
}

#[test]
fn packages_require_bridge_validation_and_report_external_dependencies() {
    use sha2::{Digest, Sha256};
    use zf_context::context_package::{
        self, ArtifactKind, BridgeArtifactValidator, BridgeDependencies, ContextPackage,
        SourceArtifact,
    };
    use zf_core::diagnostics::Diagnostic;
    struct Bridges;
    impl BridgeArtifactValidator for Bridges {
        fn validate(&self, source: &str) -> Result<BridgeDependencies, Vec<Diagnostic>> {
            assert_eq!(source, "fixture bridge source");
            Ok(BridgeDependencies {
                requires: vec!["base".into()],
                flows: vec!["research".into()],
            })
        }
    }
    let source = "fixture bridge source";
    let mut package = ContextPackage {
        version: 1,
        artifacts: vec![SourceArtifact {
            kind: ArtifactKind::Bridge,
            key: "root".into(),
            source: source.into(),
            hash: format!("{:x}", Sha256::digest(source)),
        }],
    };
    assert!(!context_package::validate_context_package(&package).valid);
    let validated = context_package::validate_package(&package, &Bridges);
    assert!(validated.valid, "{:?}", validated.diagnostics);
    assert!(
        validated
            .prerequisites
            .iter()
            .any(|p| p.kind == "bridge" && p.key == "base")
    );
    assert!(
        validated
            .prerequisites
            .iter()
            .any(|p| p.kind == "flow" && p.key == "research")
    );
    package.artifacts[0].source.push('!');
    assert_eq!(
        context_package::validate_package(&package, &Bridges).diagnostics[0].code,
        "package_hash"
    );
}

#[test]
fn producer_requests_are_data_and_preserve_captured_input_provenance() {
    use zf_context::{
        context_resources::{self, ResourceProducer},
        resources::{ContextProgram, ResourceBinding},
    };
    let strategy = ContextStrategy::new_v2("production", "Production")
        .require("input", DataType::Text)
        .require("result", DataType::Text)
        .with_program(vec![ContextBlock::emit(
            "out",
            FragmentRole::Data,
            FragmentFormat::Text,
            ContextExpr::resource("result"),
        )]);
    let producer = ResourceProducer {
        branch: "context".into(),
        route_id: "compute".into(),
        input: ContextExpr::resource("input"),
        output_pointer: None,
    };
    let program = ContextProgram {
        strategy,
        source: "".into(),
        hash: "revision".into(),
        types: TypeRegistry::new(),
        library: Default::default(),
        bindings: BTreeMap::from([("result".into(), ResourceBinding::Produced { producer })]),
        library_sources: vec![],
        type_sources: vec![],
        window: None,
    };
    let values = BTreeMap::from([("input".into(), Arc::new(json!("question")))]);
    let provenance = BTreeMap::from([(
        "input".into(),
        json!({"revision":"r1","contentRef":"opaque"}),
    )]);
    let request = context_resources::request(&program, "result", &values, &provenance).unwrap();
    assert_eq!(request.input, json!("question"));
    assert_eq!(request.sources["input"]["revision"], "r1");
    assert_eq!(request.producer.route_id, "compute");
    assert!(!values.contains_key("result"));
    assert!(context_resources::request(&program, "result", &BTreeMap::new(), &provenance).is_err());
}

#[test]
fn trial_preview_requires_complete_context_and_explicit_tool_declarations() {
    use zf_context::context::ContextCapability;
    use zf_context::request_preview::{self, PreviewTarget};
    let strategy = ContextStrategy::new_v2("trial", "Trial").capability(ContextCapability::new(
        "lookup",
        DataType::Text,
        DataType::Text,
    ));
    let program = request_preview::program(
        strategy,
        "source".into(),
        "hash".into(),
        TypeRegistry::new(),
        Default::default(),
    );
    let evaluation = context::evaluate(&program.strategy, &BTreeMap::new(), &program.types);
    let mut tools = BTreeMap::new();
    assert!(
        request_preview::validate(
            &PreviewTarget {
                provider: "fixture",
                model: "fixture",
                tools: &tools
            },
            &program,
            &evaluation
        )
        .is_err()
    );
    tools.insert(
        "lookup".into(),
        json!({"description":"Lookup", "parameters":{"type":"string"}}),
    );
    request_preview::validate(
        &PreviewTarget {
            provider: "fixture",
            model: "fixture",
            tools: &tools,
        },
        &program,
        &evaluation,
    )
    .unwrap();
    assert_eq!(evaluation.capabilities[0].id, "lookup");
}

#[test]
fn frozen_sources_preserve_open_json_and_reject_tampering() {
    use sha2::{Digest, Sha256};
    use zf_context::{context_source, frozen_context};
    let payload = json!({"toolCall":{"id":"call-7","unknown":{"signature":"keep"}}});
    let strategy =
        ContextStrategy::new_v2("frozen", "Frozen").with_program(vec![ContextBlock::emit(
            "record",
            FragmentRole::Data,
            FragmentFormat::Json,
            ContextExpr::literal(
                DataType::Record {
                    fields: BTreeMap::new(),
                },
                payload.clone(),
            ),
        )]);
    let source = context_source::generate(&strategy).unwrap();
    assert!(source.contains("use zf_context::context::*;"));
    assert_eq!(context_source::parse(&source).unwrap(), strategy);
    let mut frozen = json!({"strategy":strategy,"source":source,"hash":format!("{:x}",Sha256::digest(source.as_bytes())),"types":{},"library":{}});
    frozen_context::validate_frozen(&frozen).unwrap();
    let evaluation = context::evaluate(&strategy, &BTreeMap::new(), &TypeRegistry::new());
    assert_eq!(
        serde_json::to_value(evaluation.items).unwrap()[0]["value"],
        payload
    );
    frozen["strategy"]["name"] = json!("tampered");
    assert!(frozen_context::validate_frozen(&frozen).is_err());
    frozen["source"] = json!("tampered");
    assert!(
        frozen_context::validate_frozen(&frozen)
            .unwrap_err()
            .to_string()
            .contains("hash mismatch")
    );
}

#[test]
fn example_identity_includes_reachable_types_but_ignores_unrelated_catalogue_entries() {
    use zf_context::type_examples;
    let ty = DataType::Named {
        name: "Record".into(),
    };
    let mut types = BTreeMap::from([(
        "Record".into(),
        DataType::Record {
            fields: BTreeMap::from([("value".into(), DataType::Text)]),
        },
    )]);
    let (identity, _) = type_examples::identity(&ty, &types).unwrap();
    types.insert("Unrelated".into(), DataType::Boolean);
    assert_eq!(type_examples::identity(&ty, &types).unwrap().0, identity);
    let example = type_examples::builtin(&ty, &types).unwrap().unwrap();
    assert_eq!(example.value, json!({"value":"Texte d’exemple"}));
    types.insert(
        "Record".into(),
        DataType::Record {
            fields: BTreeMap::from([("value".into(), DataType::Number)]),
        },
    );
    assert_ne!(type_examples::identity(&ty, &types).unwrap().0, identity);
}

#[test]
fn reader_catalogue_does_not_install_or_activate_native_implementations() {
    use zf_context::resource_readers::{ReaderRegistry, standard_contracts};
    let catalogue = standard_contracts();
    let ids: Vec<_> = catalogue.iter().map(|reader| reader.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "content.json",
            "content.text",
            "file.json",
            "file.text",
            "sqlite.json"
        ]
    );
    let readers = ReaderRegistry::new();
    assert!(
        catalogue
            .iter()
            .all(|contract| !readers.contains(&contract.id))
    );
}
