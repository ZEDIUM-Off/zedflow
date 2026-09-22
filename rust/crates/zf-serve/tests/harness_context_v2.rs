use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::ContextBlock as B;
use zf_context::context::ContextExpr as E;
use zf_context::context::ContextFunction;
use zf_context::context::ContextLibrary;
use zf_context::context::ContextStrategy;
use zf_context::context::FragmentFormat as F;
use zf_context::context::FragmentRole as R;
use zf_context::context::LibraryKind;
use zf_context::context::evaluate_with_library;
use zf_context::context::validate_strategy_with_library;
use zf_context::context_source::generate;
use zf_context::context_source::generate_library;
use zf_context::context_source::generate_types;
use zf_context::context_source::parse;
use zf_context::resources::ResourceBinding;
use zf_core::types::DataType as T;
use zf_core::types::TypeRegistry;
use zf_core::types::compatible;
use zf_runtime::inference::convert_context_v1;

fn types() -> TypeRegistry {
    let shape = T::Record {
        fields: BTreeMap::from([("name".into(), T::Text)]),
    };
    TypeRegistry::from([("Term".into(), shape.clone()), ("Person".into(), shape)])
}
fn fixture() -> (ContextStrategy, ContextLibrary) {
    let library = ContextLibrary::new().projection(
        "person",
        ContextFunction::new(
            BTreeMap::from([(
                "term".into(),
                T::Named {
                    name: "Term".into(),
                },
            )]),
            T::Named {
                name: "Person".into(),
            },
            E::construct(
                "Person",
                E::record(BTreeMap::from([(
                    "name".into(),
                    E::field(E::variable("term"), "name"),
                )])),
            ),
        ),
    );
    let strategy = ContextStrategy::new_v2("construct", "Construct named value")
        .require(
            "term",
            T::Named {
                name: "Term".into(),
            },
        )
        .with_program(vec![B::emit(
            "person",
            R::Data,
            F::Json,
            E::call(
                LibraryKind::Projection,
                "person",
                BTreeMap::from([("term".into(), E::resource("term"))]),
            ),
        )]);
    (strategy, library)
}
#[test]
fn explicit_named_construction_checks_shape_values_and_keeps_nominal_boundaries() {
    let (strategy, library) = fixture();
    let registry = types();
    assert!(!compatible(
        &T::Named {
            name: "Term".into()
        },
        &T::Named {
            name: "Person".into()
        },
        &registry
    ));
    let result = evaluate_with_library(
        &strategy,
        &BTreeMap::from([("term".into(), Arc::new(json!({"name":"Ada"})))]),
        &registry,
        &library,
    );
    assert!(result.complete, "{:?}", result.diagnostics);
    let result = serde_json::to_value(&result).unwrap();
    assert_eq!(result["items"][0]["value"], json!({"name":"Ada"}));
    assert_eq!(result["items"][0]["sources"], json!(["term"]));
    let mut implicit = library.clone();
    implicit.projections.get_mut("person").unwrap().body = E::variable("term");
    assert!(
        validate_strategy_with_library(&strategy, &registry, &implicit)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "function_output")
    );
    let mut incompatible = library.clone();
    incompatible.projections.get_mut("person").unwrap().body = E::construct(
        "Person",
        E::record(BTreeMap::from([(
            "name".into(),
            E::literal(T::Number, json!(42)),
        )])),
    );
    assert!(
        validate_strategy_with_library(&strategy, &registry, &incompatible)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "construct_type")
    );
    let invalid = evaluate_with_library(
        &strategy,
        &BTreeMap::from([("term".into(), Arc::new(json!({"name":42})))]),
        &registry,
        &library,
    );
    assert!(!invalid.complete);
    assert!(invalid.items.is_empty());
    assert!(!invalid.diagnostics.is_empty());
    let mut nested = registry.clone();
    nested.insert(
        "Wrapper".into(),
        T::Record {
            fields: BTreeMap::from([(
                "person".into(),
                T::Named {
                    name: "Person".into(),
                },
            )]),
        },
    );
    let wrapping = ContextStrategy::new_v2("nested", "Nested nominal")
        .require(
            "term",
            T::Named {
                name: "Term".into(),
            },
        )
        .with_program(vec![B::emit(
            "wrapped",
            R::Data,
            F::Json,
            E::construct(
                "Wrapper",
                E::record(BTreeMap::from([("person".into(), E::resource("term"))])),
            ),
        )]);
    assert!(
        validate_strategy_with_library(&wrapping, &nested, &ContextLibrary::default()).is_err(),
        "Only explicit outer identity changes; nested Term is not a Person"
    );
}

#[test]
fn conversion_is_a_copy_preserves_legacy_source_and_demands_explicit_ambiguous_formats() {
    let legacy = ContextStrategy::new("legacy", "Legacy")
        .require(
            "history",
            T::List {
                item: Box::new(T::Record {
                    fields: BTreeMap::new(),
                }),
            },
        )
        .with_program(vec![B::emit(
            "messages",
            R::Data,
            F::Json,
            E::resource("history"),
        )]);
    let source = generate(&legacy).unwrap();
    assert!(source.starts_with("// @zedflow-context 1\n"));
    assert!(convert_context_v1(&legacy, None, &BTreeMap::new()).is_err());
    let bindings: BTreeMap<String, ResourceBinding> = serde_json::from_value(
        json!({"history":{"kind":"state","field":"history","encoding":"adkMessages"}}),
    )
    .unwrap();
    let converted = convert_context_v1(&legacy, Some(&bindings), &BTreeMap::new()).unwrap();
    assert_eq!(converted.version, 2);
    assert_eq!(converted.id, "legacy-v2");
    assert!(matches!(
        converted.program[0],
        B::Emit {
            format: F::AdkMessages,
            ..
        }
    ));
    assert_eq!(generate(&legacy).unwrap(), source);
    let mut derived = legacy.clone();
    derived.program = vec![B::emit(
        "count",
        R::Data,
        F::Json,
        E::measure(
            E::resource("history"),
            zf_context::context::MeasureUnit::Items,
        ),
    )];
    assert!(convert_context_v1(&derived, Some(&bindings), &BTreeMap::new()).is_err());
    assert!(
        convert_context_v1(
            &derived,
            Some(&bindings),
            &BTreeMap::from([("count".into(), F::Json)])
        )
        .is_ok()
    );
    assert!(
        convert_context_v1(
            &legacy,
            Some(&bindings),
            &BTreeMap::from([("unknown".into(), F::Json)])
        )
        .is_err()
    );
    let source = generate(&converted).unwrap();
    assert!(source.contains("ContextStrategy::new_v2"));
    assert!(source.starts_with("// @zedflow-context 2\n"));
    assert_eq!(parse(&source).unwrap(), converted);
    assert!(parse(&source.replacen("// @zedflow-context 2", "// @zedflow-context 1", 1)).is_err());
}

#[test]
fn exact_v2_rust_and_named_projection_compile_and_match_pure_evaluation() {
    let (mut strategy, library) = fixture();
    strategy.types = types();
    strategy.program.push(B::for_each(
        "document-excerpts",
        E::list(
            T::Text,
            vec![
                E::literal(T::Text, json!("Été🌿 en contexte")),
                E::literal(T::Text, json!("Autre document")),
            ],
        ),
        "document",
        vec![B::emit(
            "excerpt",
            R::Data,
            F::Text,
            E::truncate(E::variable("document"), 4),
        )],
    ));
    let source = generate(&strategy).unwrap();
    assert_eq!(parse(&source).unwrap(), strategy);
    if std::env::var("ZEDFLOW_TEST_CODEGEN").ok().as_deref() != Some("1") {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        r#"[package]
name="context-source-fixture"
version="0.0.0"
edition="2024"
[dependencies]
serde_json="1"
zf-core={path="zf-core"}
zf-context={path="zf-context"}
"#,
    )
    .unwrap();
    for (name, modules) in [
        ("zf-core", &["types", "diagnostics"][..]),
        ("zf-context", &["context", "context_library"][..]),
    ] {
        let target = root.path().join(name);
        std::fs::create_dir_all(target.join("src")).unwrap();
        let dependency = if name == "zf-context" {
            "zf-core={path=\"../zf-core\"}\n"
        } else {
            ""
        };
        std::fs::write(target.join("Cargo.toml"), format!("[package]\nname=\"{name}\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[dependencies]\nserde={{version=\"1\",features=[\"derive\"]}}\nserde_json=\"1\"\n{dependency}")).unwrap();
        std::fs::write(
            target.join("src/lib.rs"),
            modules
                .iter()
                .map(|module| format!("pub mod {module};\n"))
                .collect::<String>(),
        )
        .unwrap();
        for module in modules {
            std::fs::copy(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../{name}/src/{module}.rs")),
                target.join(format!("src/{module}.rs")),
            )
            .unwrap();
        }
    }
    std::fs::write(src.join("strategy.rs"), source).unwrap();
    std::fs::write(src.join("library.rs"), generate_library(&library).unwrap()).unwrap();
    std::fs::write(src.join("types.rs"), generate_types(&types()).unwrap()).unwrap();
    std::fs::write(src.join("main.rs"),r#"mod strategy;mod library;mod types;
fn main(){let resources=std::collections::BTreeMap::from([("term".into(),std::sync::Arc::new(serde_json::json!({"name":"Ada"})))]);let result=zf_context::context::evaluate_with_library(&strategy::strategy(),&resources,&types::types(),&library::library());assert!(result.complete);println!("{}",serde_json::to_string(&result).unwrap());}"#).unwrap();
    let output = std::process::Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(root.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", "/tmp/zedflow-adk-target")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = evaluate_with_library(
        &strategy,
        &BTreeMap::from([("term".into(), Arc::new(json!({"name":"Ada"})))]),
        &types(),
        &library,
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
