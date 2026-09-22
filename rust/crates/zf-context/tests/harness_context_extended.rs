use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::Comparison;
use zf_context::context::ContextBlock as B;
use zf_context::context::ContextExpr as E;
use zf_context::context::ContextFunction;
use zf_context::context::ContextItem;
use zf_context::context::ContextLibrary;
use zf_context::context::ContextPredicate as P;
use zf_context::context::ContextStrategy;
use zf_context::context::ContextValue;
use zf_context::context::EvaluationLimits;
use zf_context::context::FragmentFormat as F;
use zf_context::context::FragmentRole as R;
use zf_context::context::LibraryKind;
use zf_context::context::MeasureUnit;
use zf_context::context::evaluate;
use zf_context::context::evaluate_with_library;
use zf_context::context::evaluate_with_limits;
use zf_context::context::validate_strategy;
use zf_context::context::validate_strategy_with_library;
use zf_context::context::validate_structure;
use zf_context::context_source::generate;
use zf_context::context_source::generate_library;
use zf_context::context_source::parse;
use zf_context::context_source::parse_library;
use zf_core::types::DataType as T;
use zf_core::types::TypeRegistry;
fn text(value: &str) -> E {
    E::literal(T::Text, json!(value))
}
fn number(value: i64) -> E {
    E::literal(T::Number, json!(value))
}
fn field(name: &str) -> E {
    E::field(E::variable("row"), name)
}
fn row_type() -> T {
    T::Record {
        fields: BTreeMap::from([
            ("name".into(), T::Text),
            ("score".into(), T::Number),
            ("team".into(), T::Text),
        ]),
    }
}
fn inputs() -> BTreeMap<String, Arc<Value>> {
    BTreeMap::from([(
        "rows".into(),
        Arc::new(json!([
            {"name":"Ada","score":9,"team":"red","private":"x".repeat(100)},
            {"name":"Bob","score":3,"team":"blue"},
            {"name":"Cy","score":10,"team":"red"},
            {"name":"Dee","score":9,"team":"blue"}
        ])),
    )])
}
fn strategy(program: Vec<B>) -> ContextStrategy {
    ContextStrategy::new("extended", "Extended context")
        .require(
            "rows",
            T::List {
                item: Box::new(row_type()),
            },
        )
        .with_program(program)
}
fn emit(id: &str, value: E) -> B {
    B::emit(id, R::Data, F::Json, value)
}
fn library() -> ContextLibrary {
    ContextLibrary::new()
        .projection(
            "label",
            ContextFunction::new(
                BTreeMap::from([("person".into(), row_type())]),
                T::Text,
                E::template(
                    "Hello {{name}}",
                    BTreeMap::from([("name".into(), E::field(E::variable("person"), "name"))]),
                ),
            ),
        )
        .subprogram(
            "greet",
            ContextFunction::new(
                BTreeMap::from([("value".into(), row_type())]),
                T::Text,
                E::call(
                    LibraryKind::Projection,
                    "label",
                    BTreeMap::from([("person".into(), E::variable("value"))]),
                ),
            ),
        )
}
fn sample() -> ContextStrategy {
    let rows = E::resource("rows");
    let selected = E::take(
        E::sort(
            E::filter(
                rows.clone(),
                "row",
                P::compare(field("score"), Comparison::Gte, number(9)),
            ),
            "row",
            field("score"),
            true,
        ),
        2,
    );
    strategy(vec![
        emit(
            "selected",
            E::map(
                selected,
                "row",
                E::template("{{name}}", BTreeMap::from([("name".into(), field("name"))])),
            ),
        ),
        B::group(
            "views",
            "Views",
            vec![
                emit("grouped", E::group_by(rows.clone(), "row", field("team"))),
                emit("unique", E::dedup(rows.clone(), "row", field("team"))),
                emit(
                    "records",
                    E::map(
                        rows.clone(),
                        "row",
                        E::record(BTreeMap::from([("label".into(), field("name"))])),
                    ),
                ),
                emit(
                    "greetings",
                    E::map(
                        rows.clone(),
                        "row",
                        E::call(
                            LibraryKind::Subprogram,
                            "greet",
                            BTreeMap::from([("value".into(), E::variable("row"))]),
                        ),
                    ),
                ),
            ],
        ),
        B::branch(
            "budget",
            P::all(vec![
                P::compare(
                    E::measure(rows.clone(), MeasureUnit::Items),
                    Comparison::Lte,
                    number(4),
                ),
                P::contains(text("étiquette"), text("éti")),
                P::contains(E::map(rows.clone(), "row", field("name")), text("Ada")),
                P::compare(number(1), Comparison::Ne, number(2)),
            ]),
            vec![emit("encoded", E::to_json(E::take(rows, 1)))],
            vec![],
        ),
        emit("bytes", E::measure(text("é😀"), MeasureUnit::Bytes)),
        emit(
            "media",
            E::measure(
                E::literal(
                    T::Media {
                        media_type: "image/png".into(),
                    },
                    json!({"mediaType":"image/png","contentRef":"sha256:fixture"}),
                ),
                MeasureUnit::Media,
            ),
        ),
    ])
}
#[test]
fn collections_templates_calls_and_units_evaluate_in_stable_order_with_shared_views() {
    let resources = inputs();
    let result = evaluate_with_library(&sample(), &resources, &TypeRegistry::new(), &library());
    assert!(result.complete, "{:?}", result.diagnostics);
    let wire = serde_json::to_value(&result).unwrap();
    assert_eq!(wire["items"][0]["value"], json!(["Cy", "Ada"]));
    assert_eq!(wire["items"][1]["items"][0]["value"][0]["key"], "red");
    assert_eq!(
        wire["items"][1]["items"][0]["value"][0]["items"][1]["name"],
        "Cy"
    );
    assert_eq!(wire["items"][1]["items"][1]["value"][1]["name"], "Bob");
    assert_eq!(
        wire["items"][1]["items"][3]["value"],
        json!(["Hello Ada", "Hello Bob", "Hello Cy", "Hello Dee"])
    );
    assert_eq!(wire["items"][3]["value"], 6);
    assert_eq!(wire["items"][4]["value"], 1);
    assert_eq!(result.reads, ["rows"]);
    let ContextItem::Group { items, .. } = &result.items[1] else {
        panic!()
    };
    let ContextItem::Fragment {
        value: ContextValue::Array { items },
        ..
    } = &items[2]
    else {
        panic!()
    };
    let ContextValue::Object { fields } = &items[0] else {
        panic!()
    };
    let ContextValue::Shared { root, pointer } = &fields["label"] else {
        panic!()
    };
    assert!(Arc::ptr_eq(root, &resources["rows"]));
    assert_eq!(pointer, "/0/name");
}
#[test]
fn source_roundtrip_covers_extended_forms_and_explicit_libraries_without_executing_code() {
    let strategy = sample();
    let library = library();
    let source = generate(&strategy).unwrap();
    assert_eq!(parse(&source).unwrap(), strategy);
    let source_library = generate_library(&library).unwrap();
    assert_eq!(parse_library(&source_library).unwrap(), library);
    assert!(!source_library.contains("serde_json::from_str"));
    for modified in [
        source.replace("2usize", "std::process::id() as usize"),
        source.replace(
            "ContextExpr::variable(\"row\")",
            "ContextExpr::variable(include_str!(\"/etc/passwd\"))",
        ),
        source.replace("Comparison::Gte", "unknown()"),
    ] {
        assert!(parse(&modified).is_err());
    }
    assert!(
        parse_library(&source_library.replace(
            "ContextLibrary::new()",
            "{ panic!(); ContextLibrary::new() }"
        ))
        .is_err()
    );
    assert!(parse_library(&source_library.replace(".projection(", ".hidden(")).is_err());
    assert!(!evaluate(&strategy, &inputs(), &TypeRegistry::new()).complete);
}
#[test]
fn types_and_variable_scopes_have_no_implicit_coercion() {
    let bad = [
        emit(
            "bad",
            E::template("{{n}}", BTreeMap::from([("n".into(), number(1))])),
        ),
        emit("bad", E::variable("row")),
        emit(
            "bad",
            E::sort(E::resource("rows"), "row", E::variable("row"), false),
        ),
        emit("bad", E::measure(text("hello"), MeasureUnit::Items)),
        B::branch(
            "bad",
            P::compare(text("1"), Comparison::Gt, number(0)),
            vec![],
            vec![],
        ),
        B::branch("bad", P::contains(text("1"), number(1)), vec![], vec![]),
    ];
    for block in bad {
        assert!(validate_strategy(&strategy(vec![block]), &TypeRegistry::new()).is_err());
    }
    let wrong = strategy(vec![emit(
        "bad",
        E::call(
            LibraryKind::Subprogram,
            "greet",
            BTreeMap::from([("value".into(), text("wrong"))]),
        ),
    )]);
    assert!(
        validate_strategy_with_library(&wrong, &TypeRegistry::new(), &library())
            .unwrap_err()
            .iter()
            .any(|d| d.code == "function_argument_type")
    );
    // Unlinked source remains editable; only a linked evaluation can resolve its catalogue.
    validate_structure(&sample()).unwrap();
}
#[test]
fn parameterized_function_cycles_and_ambient_resource_reads_are_rejected() {
    let mut library = ContextLibrary::new().subprogram(
        "loop",
        ContextFunction::new(
            BTreeMap::new(),
            T::Text,
            E::call(LibraryKind::Subprogram, "loop", BTreeMap::new()),
        ),
    );
    let empty = ContextStrategy::new("test", "Test");
    let errors =
        validate_strategy_with_library(&empty, &TypeRegistry::new(), &library).unwrap_err();
    assert!(errors.iter().any(|d| d.code == "function_cycle"));
    library.subprograms.get_mut("loop").unwrap().body = E::resource("rows");
    assert!(
        validate_strategy_with_library(&empty, &TypeRegistry::new(), &library)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "function_resource")
    );
    library.subprograms.get_mut("loop").unwrap().body = E::variable("outside");
    assert!(
        validate_strategy_with_library(&empty, &TypeRegistry::new(), &library)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "unknown_variable")
    );
}
#[test]
fn reads_and_demands_only_include_expressions_reached_during_iteration() {
    let program = strategy(vec![emit(
        "filtered",
        E::filter(
            E::resource("rows"),
            "row",
            P::present(E::resource("missing")),
        ),
    )])
    .require("missing", T::Text);
    let mut resources = inputs();
    resources.insert("rows".into(), Arc::new(json!([])));
    let empty = evaluate(&program, &resources, &TypeRegistry::new());
    assert!(empty.complete);
    assert_eq!(empty.reads, ["rows"]);
    let result = evaluate(&program, &inputs(), &TypeRegistry::new());
    assert!(result.complete);
    assert_eq!(result.reads, ["missing", "rows"]);
    assert!(result.needs.is_empty());
    let demanded = strategy(vec![emit(
        "filtered",
        E::filter(
            E::resource("rows"),
            "row",
            P::equal(E::resource("missing"), text("yes")),
        ),
    )])
    .require("missing", T::Text);
    let result = evaluate(&demanded, &inputs(), &TypeRegistry::new());
    assert_eq!(result.needs[0].resource, "missing");
}
#[test]
fn numeric_order_preserves_large_integers_and_sort_ties_remain_stable() {
    let program = ContextStrategy::new("numeric", "Numeric").with_program(vec![
        B::branch(
            "large",
            P::compare(
                E::literal(T::Number, json!(u64::MAX)),
                Comparison::Lt,
                E::literal(T::Number, json!(18446744073709551616.0f64)),
            ),
            vec![emit("matched", number(1))],
            vec![],
        ),
        emit(
            "sorted",
            E::sort(
                E::literal(
                    T::List {
                        item: Box::new(T::Number),
                    },
                    json!([9007199254740993u64, 9007199254740992.0, 9007199254740992u64]),
                ),
                "n",
                E::variable("n"),
                false,
            ),
        ),
    ]);
    let result = evaluate(&program, &BTreeMap::new(), &TypeRegistry::new());
    assert!(result.complete, "{:?}", result.diagnostics);
    let output = serde_json::to_value(result).unwrap();
    assert_eq!(output["items"][0]["value"], 1);
    assert_eq!(
        output["items"][1]["value"],
        json!([9007199254740992.0, 9007199254740992u64, 9007199254740993u64])
    );
}
#[test]
fn evaluation_limits_bound_iteration_and_derived_bytes_without_clock_thresholds() {
    let program = ContextStrategy::new("bounded", "Bounded")
        .require(
            "values",
            T::List {
                item: Box::new(T::Number),
            },
        )
        .with_program(vec![emit(
            "mapped",
            E::map(
                E::resource("values"),
                "n",
                E::template("012345678901234567890123456789", BTreeMap::new()),
            ),
        )]);
    let resources = BTreeMap::from([(
        "values".into(),
        Arc::new(json!((0..20).collect::<Vec<_>>())),
    )]);
    for limits in [
        EvaluationLimits {
            max_steps: 10,
            max_items: 100,
            max_bytes: 10000,
        },
        EvaluationLimits {
            max_steps: 1000,
            max_items: 10,
            max_bytes: 10000,
        },
        EvaluationLimits {
            max_steps: 1000,
            max_items: 100,
            max_bytes: 128,
        },
    ] {
        let result = evaluate_with_limits(
            &program,
            &resources,
            &TypeRegistry::new(),
            &ContextLibrary::default(),
            limits,
        );
        assert!(!result.complete);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "evaluation_limit")
        );
        assert!(result.items.is_empty());
    }
}

#[test]
fn exact_extended_sources_and_library_compile_and_match_evaluation() {
    if std::env::var_os("ZEDFLOW_TEST_CODEGEN").is_none() {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let src = directory.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        directory.path().join("Cargo.toml"),
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
        let target = directory.path().join(name);
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
    let source = generate(&sample()).unwrap();
    let library_source = generate_library(&library()).unwrap();
    let type_source = zf_context::context_source::generate_types(&TypeRegistry::from([(
        "Row".into(),
        row_type(),
    )]))
    .unwrap();
    std::fs::write(src.join("generated.rs"), &source).unwrap();
    std::fs::write(src.join("generated_library.rs"), &library_source).unwrap();
    std::fs::write(src.join("generated_types.rs"), &type_source).unwrap();
    let values: BTreeMap<_, _> = inputs()
        .into_iter()
        .map(|(k, v)| (k, v.as_ref().clone()))
        .collect();
    std::fs::write(
        src.join("inputs.json"),
        serde_json::to_vec(&values).unwrap(),
    )
    .unwrap();
    std::fs::write(src.join("main.rs"), r#"mod generated; mod generated_library; mod generated_types;
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::evaluate_with_library;
use zf_core::types::TypeRegistry;
fn main() {
    let inputs: BTreeMap<String,serde_json::Value> = serde_json::from_str(include_str!("inputs.json")).unwrap();
    let resources = inputs.into_iter().map(|(k,v)| (k,Arc::new(v))).collect();
    let types = generated_types::types();
    zf_core::types::validate_type(&zf_core::types::DataType::Named { name: "Row".into() }, &types).unwrap();
    println!("{}",serde_json::to_string(&evaluate_with_library(&generated::strategy(), &resources, &TypeRegistry::new(), &generated_library::library())).unwrap());
}"#).unwrap();
    let output = std::process::Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.path().join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", "/tmp/zedflow-adk-target")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(src.join("generated.rs")).unwrap(),
        source
    );
    assert_eq!(
        std::fs::read_to_string(src.join("generated_library.rs")).unwrap(),
        library_source
    );
    assert_eq!(
        std::fs::read_to_string(src.join("generated_types.rs")).unwrap(),
        type_source
    );
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual,
        serde_json::to_value(evaluate_with_library(
            &sample(),
            &inputs(),
            &TypeRegistry::new(),
            &library()
        ))
        .unwrap()
    );
}
