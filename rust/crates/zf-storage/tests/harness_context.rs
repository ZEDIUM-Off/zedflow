use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::ContextBlock as Block;
use zf_context::context::ContextCapability;
use zf_context::context::ContextExpr as Expr;
use zf_context::context::ContextItem;
use zf_context::context::ContextPredicate as Predicate;
use zf_context::context::ContextStrategy;
use zf_context::context::ContextValue;
use zf_context::context::FragmentFormat as Format;
use zf_context::context::FragmentRole as Role;
use zf_context::context::evaluate;
use zf_context::context::validate_strategy;
use zf_context::context::validate_structure;
use zf_context::context_source::generate;
use zf_context::context_source::parse;
use zf_core::types::DataType as Ty;
use zf_core::types::TypeRegistry;
use zf_storage::context_store::Conflict;
use zf_storage::context_store::ContextStore;

fn record(fields: &[(&str, Ty)]) -> Ty {
    Ty::Record {
        fields: fields
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
    }
}
fn report_type() -> Ty {
    record(&[
        ("failed", Ty::Boolean),
        ("summary", Ty::Text),
        ("stats", record(&[("count", Ty::Number)])),
    ])
}
fn registry() -> TypeRegistry {
    BTreeMap::from([("Review".into(), report_type())])
}
fn sample() -> ContextStrategy {
    ContextStrategy::new("working-system", "Contexte — 工作系统")
        .require("question", Ty::Text)
        .require("review", Ty::Named {name: "Review".into()})
        .require("image", Ty::Media {media_type: "image/png".into()})
        .capability(ContextCapability::new("read", record(&[("path", Ty::Text)]), Ty::Text))
        .with_program(vec![
            Block::group("profile", "Instructions", vec![Block::emit("role", Role::Instruction, Format::Text, Expr::literal(Ty::Text, json!("Keep {{input}} literal. Quotes \" ; newline\n ; nul\0 ; €")))]),
            Block::emit("question", Role::Data, Format::Text, Expr::resource("question")),
            Block::branch("review-when", Predicate::all(vec![Predicate::present(Expr::resource("review")), Predicate::any(vec![Predicate::equal(Expr::field(Expr::resource("review"), "failed"), Expr::literal(Ty::Boolean, json!(true))), Predicate::negate(Predicate::present(Expr::resource("question")))])]), vec![
                Block::group("review-group", "Revue", vec![
                    Block::emit("review-fields", Role::Data, Format::Json, Expr::project(Expr::resource("review"), &["summary", "stats"])),
                    Block::emit("review-summary", Role::Data, Format::Text, Expr::field(Expr::resource("review"), "summary")),
                ]),
            ], vec![Block::emit("no-review", Role::Data, Format::Text, Expr::literal(Ty::Text, json!("Pas de revue en échec")))]),
            Block::branch("image-when", Predicate::present(Expr::resource("image")), vec![Block::emit("image", Role::Data, Format::Media, Expr::resource("image"))], vec![]),
            Block::emit("limits", Role::Data, Format::Json, Expr::literal(record(&[("minimum", Ty::Number), ("maximum", Ty::Number), ("values", Ty::List {item: Box::new(Ty::Number)})]), json!({"minimum":i64::MIN,"maximum":u64::MAX,"values":[5_000_000_000i64, -2.5f64, 1.0e30f64],"extra":[null,true,"é\0"]}))),
        ])
}
fn inputs() -> BTreeMap<String, Arc<Value>> {
    BTreeMap::from([
        (
            "question".into(),
            Arc::new(json!("Inspecter la modification")),
        ),
        (
            "review".into(),
            Arc::new(
                json!({"failed":true,"summary":"Une erreur","stats":{"count":1},"unusedLargePayload":"x".repeat(50_000)}),
            ),
        ),
        (
            "image".into(),
            Arc::new(json!({"contentRef":"sha256:fixture","mediaType":"image/png"})),
        ),
    ])
}

#[test]
fn structured_rust_round_trips_every_supported_form_and_unbound_named_types() {
    let strategy = sample();
    validate_structure(&strategy).unwrap();
    assert!(validate_strategy(&strategy, &TypeRegistry::new()).is_err());
    validate_strategy(&strategy, &registry()).unwrap();
    let source = generate(&strategy).unwrap();
    assert!(source.contains("ContextPredicate::all"));
    assert!(source.contains("ContextExpr::project"));
    assert!(source.contains("18446744073709551615u64"));
    assert!(source.contains("-9223372036854775808i64"));
    assert!(!source.contains("serde_json::from_str"));
    assert_eq!(parse(&source).unwrap(), strategy);
    assert_eq!(
        serde_json::from_value::<ContextStrategy>(serde_json::to_value(&strategy).unwrap())
            .unwrap(),
        strategy
    );
    // Formatting and ordinary comments do not change the program.
    assert_eq!(
        parse(&source.replace(
            ".with_program",
            "// Recorded program\n        .with_program"
        ))
        .unwrap(),
        strategy
    );
}

#[test]
fn source_rejects_arbitrary_rust_attributes_macros_and_duplicate_requirements() {
    let source = generate(&sample()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("must-not-exist");
    let code = format!(
        "std::fs::write({:?}, b\"executed\").unwrap();",
        marker.to_str().unwrap()
    );
    for altered in [
        source.replace(
            "    ContextStrategy::new",
            &format!("    {code}\n    ContextStrategy::new"),
        ),
        source.replace(
            "Expr::resource(\"question\")",
            "Expr::resource(std::env::var(\"SECRET\").unwrap().as_str())",
        ),
        source.replace("pub fn strategy", "#[cfg(any())]\npub fn strategy"),
        source.replace("DataType::Text)", "#[cfg(any())] DataType::Text)"),
        source.replace("5000000000i64", "std::process::id()"),
        source.replace("5000000000i64", "9223372036854775808i64"),
        source.replace("5000000000i64", "-1u64"),
        source.replace("5000000000i64", "5000000000"),
        source.replace("5000000000i64", "1.0f32"),
        source.replace(
            ".with_program",
            ".require(\"question\", DataType::Text)\n        .with_program",
        ),
        format!("{source}\nfn hidden() {{ {code} }}"),
    ] {
        assert_ne!(altered, source);
        assert!(
            parse(&altered).is_err(),
            "Accepted unexpected source: {altered}"
        );
    }
    assert!(!marker.exists());
}

#[test]
fn validation_rejects_unknown_fields_coercion_and_incompatible_fragment_representations() {
    let expression = Expr::field(Expr::resource("review"), "missing");
    let bad = ContextStrategy::new("invalid", "Invalid")
        .require("review", report_type())
        .with_program(vec![Block::emit(
            "bad",
            Role::Data,
            Format::Text,
            expression,
        )]);
    assert!(
        validate_strategy(&bad, &registry())
            .unwrap_err()
            .iter()
            .any(|error| error.code == "unknown_field")
    );
    let bad = ContextStrategy::new("invalid", "Invalid").with_program(vec![Block::branch(
        "bad",
        Predicate::equal(
            Expr::literal(Ty::Number, json!(1)),
            Expr::literal(Ty::Text, json!("1")),
        ),
        vec![],
        vec![],
    )]);
    assert!(
        validate_strategy(&bad, &registry())
            .unwrap_err()
            .iter()
            .any(|error| error.code == "comparison_type")
    );
    for (role, format, ty) in [
        (Role::Data, Format::Text, Ty::Number),
        (Role::Instruction, Format::Json, Ty::Text),
        (
            Role::Data,
            Format::Json,
            Ty::Media {
                media_type: "image/png".into(),
            },
        ),
    ] {
        let bad = ContextStrategy::new("invalid", "Invalid")
            .require("value", ty)
            .with_program(vec![Block::emit(
                "bad",
                role,
                format,
                Expr::resource("value"),
            )]);
        assert!(validate_strategy(&bad, &registry()).is_err());
    }
    let bad = ContextStrategy::new("invalid", "Invalid").with_program(vec![Block::branch(
        "bad",
        Predicate::all(vec![]),
        vec![],
        vec![],
    )]);
    assert!(
        validate_structure(&bad)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "empty_predicate")
    );
}

#[test]
fn projections_share_input_payloads_and_each_evaluation_gets_a_fresh_context() {
    let strategy = sample();
    let mut resources = inputs();
    let result = evaluate(&strategy, &resources, &registry());
    assert!(result.complete, "{:?}", result.diagnostics);
    let ContextItem::Group { items, .. } = &result.items[2] else {
        panic!("Review group missing")
    };
    let ContextItem::Fragment {
        value: ContextValue::Object { fields },
        sources,
        ..
    } = &items[0]
    else {
        panic!("Projection missing")
    };
    let ContextValue::Shared { root, pointer } = &fields["summary"] else {
        panic!("Expected shared field view")
    };
    assert!(Arc::ptr_eq(root, &resources["review"]));
    assert_eq!(pointer, "/summary");
    assert_eq!(sources, &["review"]);
    let wire = serde_json::to_value(&result).unwrap();
    assert_eq!(
        wire["items"][2]["items"][0]["value"],
        json!({"summary":"Une erreur","stats":{"count":1}})
    );
    assert!(!wire.to_string().contains("unusedLargePayload"));
    assert_eq!(wire["items"][3]["value"]["mediaType"], "image/png");
    resources.insert(
        "review".into(),
        Arc::new(json!({"failed":false,"summary":"Corrigé","stats":{"count":0}})),
    );
    let next = serde_json::to_value(evaluate(&strategy, &resources, &registry())).unwrap();
    assert_eq!(next["items"][2]["id"], "no-review");
    assert_eq!(root["summary"], "Une erreur");
}

#[test]
fn only_consumed_missing_resources_request_a_producer_and_presence_short_circuits() {
    let mut resources = inputs();
    resources.remove("review");
    resources.remove("image");
    let result = evaluate(&sample(), &resources, &registry());
    assert!(result.complete);
    assert!(result.needs.is_empty());
    resources.remove("question");
    let result = evaluate(&sample(), &resources, &registry());
    assert!(!result.complete);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.needs.len(), 1);
    assert_eq!(result.needs[0].resource, "question");
    assert_eq!(result.needs[0].data_type, Ty::Text);
    assert_eq!(result.needs[0].required_by, vec!["program[1]"]);
    // Capabilities are returned as requested descriptors, not invoked or granted.
    assert_eq!(result.capabilities[0].id, "read");
}

#[test]
fn malformed_values_report_the_resource_boundary_only_when_it_is_read() {
    let strategy = ContextStrategy::new("condition", "Conditional source")
        .require("enabled", Ty::Boolean)
        .require("payload", Ty::Text)
        .with_program(vec![Block::branch(
            "gate",
            Predicate::equal(
                Expr::resource("enabled"),
                Expr::literal(Ty::Boolean, json!(true)),
            ),
            vec![Block::emit(
                "data",
                Role::Data,
                Format::Text,
                Expr::resource("payload"),
            )],
            vec![],
        )]);
    let mut resources = BTreeMap::from([
        ("enabled".into(), Arc::new(json!(false))),
        ("payload".into(), Arc::new(json!({"wrong":"type"}))),
    ]);
    assert!(evaluate(&strategy, &resources, &registry()).complete);
    resources.insert("enabled".into(), Arc::new(json!(true)));
    let result = evaluate(&strategy, &resources, &registry());
    assert!(!result.complete);
    assert!(result.needs.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|error| error.code == "value_type" && error.path.contains("resources.payload"))
    );
    let mut resources = inputs();
    resources.insert(
        "image".into(),
        Arc::new(json!({"mediaType":"image/jpeg","contentRef":"blob"})),
    );
    assert!(!evaluate(&sample(), &resources, &registry()).complete);
}

#[test]
fn escaped_field_names_keep_their_identity_through_nested_projection_and_equality() {
    let ty = record(&[("a/b~c", Ty::Text)]);
    let expression = Expr::field(Expr::project(Expr::resource("record"), &["a/b~c"]), "a/b~c");
    let strategy = ContextStrategy::new("escaped", "Escaped paths")
        .require("record", ty.clone())
        .with_program(vec![
            Block::emit("value", Role::Data, Format::Text, expression),
            Block::branch(
                "compare",
                Predicate::equal(
                    Expr::project(Expr::resource("record"), &["a/b~c"]),
                    Expr::literal(ty, json!({"a/b~c":"correct"})),
                ),
                vec![Block::emit(
                    "matched",
                    Role::Data,
                    Format::Text,
                    Expr::literal(Ty::Text, json!("yes")),
                )],
                vec![],
            ),
        ]);
    let input = Arc::new(json!({"a/b~c":"correct","extra":"ignored by projection"}));
    let result = evaluate(
        &strategy,
        &BTreeMap::from([("record".into(), Arc::clone(&input))]),
        &registry(),
    );
    assert!(result.complete);
    assert_eq!(result.items.len(), 2);
    let ContextItem::Fragment {
        value: ContextValue::Shared { root, pointer },
        ..
    } = &result.items[0]
    else {
        panic!("Missing field")
    };
    assert!(Arc::ptr_eq(root, &input));
    assert_eq!(pointer, "/a~1b~0c");
}

#[tokio::test]
async fn catalog_keeps_invalid_sources_visible_and_detects_external_changes() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ContextStore::new(workspace.path().into());
    assert!(store.list().await.unwrap().is_empty());
    let original = store.save(&sample(), None).await.unwrap();
    assert!(
        original
            .path
            .starts_with(workspace.path().join(".zedflow/context"))
    );
    assert_eq!(original.strategy.as_ref(), Some(&sample()));
    assert_eq!(
        store.read("working-system").await.unwrap().source,
        original.source
    );
    assert!(store.list().await.unwrap()[0].source.is_none());
    let invalid = workspace.path().join(".zedflow/context/invalid.rs");
    std::fs::write(&invalid, "fn arbitrary() { panic!(); }").unwrap();
    let binary = workspace.path().join(".zedflow/context/binary.rs");
    std::fs::write(binary, [0xff, 0xfe]).unwrap();
    let files = store.list().await.unwrap();
    assert_eq!(files.len(), 3);
    assert!(
        files
            .iter()
            .filter(|file| file.key != "working-system")
            .all(|file| file.strategy.is_none() && !file.diagnostics.is_empty())
    );
    let edited = original
        .source
        .unwrap()
        .replace("Contexte — 工作系统", "Modification externe");
    std::fs::write(&original.path, &edited).unwrap();
    let error = store
        .save(&sample(), Some(&original.hash))
        .await
        .unwrap_err();
    assert!(error.downcast_ref::<Conflict>().is_some());
    assert_eq!(std::fs::read_to_string(&original.path).unwrap(), edited);
    assert!(store.read("../outside").await.is_err());
}

#[tokio::test]
async fn independent_store_instances_cannot_both_replace_the_same_loaded_hash() {
    let workspace = tempfile::tempdir().unwrap();
    let initial = ContextStore::new(workspace.path().into())
        .save(&sample(), None)
        .await
        .unwrap();
    let left = ContextStore::new(workspace.path().into());
    let right = ContextStore::new(workspace.path().into());
    let mut a = sample();
    a.name = "Left revision".into();
    let mut b = sample();
    b.name = "Right revision".into();
    let (a, b) = tokio::join!(
        left.save(&a, Some(&initial.hash)),
        right.save(&b, Some(&initial.hash))
    );
    let (saved, rejected) = match (a, b) {
        (Ok(a), Err(b)) => (a, b),
        (Err(a), Ok(b)) => (b, a),
        other => panic!("Expected exactly one commit: {other:?}"),
    };
    assert!(rejected.downcast_ref::<Conflict>().is_some());
    assert_eq!(left.read("working-system").await.unwrap().hash, saved.hash);
    let temporary = std::fs::read_dir(workspace.path().join(".zedflow/context"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(temporary, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn context_catalog_never_follows_directory_or_source_symlinks() {
    use std::os::unix::fs::symlink;
    let outside = tempfile::tempdir().unwrap();
    for component in [".zedflow", ".zedflow/context"] {
        let workspace = tempfile::tempdir().unwrap();
        if component.contains('/') {
            std::fs::create_dir(workspace.path().join(".zedflow")).unwrap();
        }
        symlink(outside.path(), workspace.path().join(component)).unwrap();
        let store = ContextStore::new(workspace.path().into());
        assert!(store.list().await.is_err());
        assert!(store.save(&sample(), None).await.is_err());
    }
    assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none());
    let workspace = tempfile::tempdir().unwrap();
    let store = ContextStore::new(workspace.path().into());
    let original = store.save(&sample(), None).await.unwrap();
    let target = outside.path().join("outside.rs");
    std::fs::write(&target, original.source.unwrap()).unwrap();
    std::fs::remove_file(&original.path).unwrap();
    symlink(&target, &original.path).unwrap();
    assert!(
        store
            .read("working-system")
            .await
            .unwrap()
            .strategy
            .is_none()
    );
    assert!(store.save(&sample(), Some(&original.hash)).await.is_err());
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        generate(&sample()).unwrap()
    );
}

#[tokio::test]
async fn mismatched_identity_and_oversized_sources_remain_diagnostics() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ContextStore::new(workspace.path().into());
    let original = store.save(&sample(), None).await.unwrap();
    std::fs::write(
        workspace.path().join(".zedflow/context/other.rs"),
        original.source.unwrap(),
    )
    .unwrap();
    std::fs::write(
        workspace.path().join(".zedflow/context/large.rs"),
        vec![b'x'; 1024 * 1024 + 1],
    )
    .unwrap();
    let files = store.list().await.unwrap();
    assert!(
        files
            .iter()
            .find(|file| file.key == "other")
            .unwrap()
            .diagnostics
            .iter()
            .any(|error| error.code == "context_identity")
    );
    assert!(
        files
            .iter()
            .find(|file| file.key == "large")
            .unwrap()
            .strategy
            .is_none()
    );
}

#[test]
fn exact_generated_rust_compiles_and_matches_the_daemon_evaluation() {
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
    std::fs::write(src.join("generated.rs"), &source).unwrap();
    assert_eq!(
        std::fs::read_to_string(src.join("generated.rs")).unwrap(),
        source
    );
    let main = r#"mod generated;
use std::{collections::BTreeMap, sync::Arc};
use zf_context::context::evaluate;
use zf_core::types::TypeRegistry;
fn main() {
    let (types, values): (TypeRegistry, Vec<BTreeMap<String, serde_json::Value>>) = serde_json::from_str(include_str!("cases.json")).unwrap();
    let strategy = generated::strategy();
    let results: Vec<_> = values.into_iter().map(|values| {
        let resources = values.into_iter().map(|(key, value)| (key, Arc::new(value))).collect();
        serde_json::to_value(evaluate(&strategy, &resources, &types)).unwrap()
    }).collect();
    println!("{}", serde_json::to_string(&results).unwrap());
}
"#;
    std::fs::write(src.join("main.rs"), main).unwrap();
    let cases = [
        inputs(),
        BTreeMap::from([("question".into(), Arc::new(json!("Second passage")))]),
    ];
    let values: Vec<BTreeMap<_, _>> = cases
        .iter()
        .map(|case| {
            case.iter()
                .map(|(key, value)| (key.clone(), value.as_ref().clone()))
                .collect()
        })
        .collect();
    std::fs::write(
        src.join("cases.json"),
        serde_json::to_vec(&(registry(), values)).unwrap(),
    )
    .unwrap();
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
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected: Vec<_> = cases
        .iter()
        .map(|case| serde_json::to_value(evaluate(&sample(), case, &registry())).unwrap())
        .collect();
    assert_eq!(actual, json!(expected));
}
