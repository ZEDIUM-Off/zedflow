use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zf_context::context::ContextBlock as B;
use zf_context::context::ContextExpr as E;
use zf_context::context::ContextLibrary;
use zf_context::context::ContextPredicate as P;
use zf_context::context::ContextStrategy;
use zf_context::context::EvaluationLimits;
use zf_context::context::FragmentFormat as F;
use zf_context::context::FragmentRole as R;
use zf_context::context::evaluate;
use zf_context::context::evaluate_with_limits;
use zf_context::context::validate_strategy;
use zf_context::context_source::generate;
use zf_context::context_source::parse;
use zf_core::types::DataType as T;
use zf_core::types::TypeRegistry;

fn literal(text: &str) -> E {
    E::literal(T::Text, json!(text))
}
fn emit(id: &str, value: E) -> B {
    B::emit(id, R::Data, F::Text, value)
}
fn term() -> T {
    T::Record {
        fields: BTreeMap::from([("name".into(), T::Text), ("definition".into(), T::Text)]),
    }
}
fn strategy() -> ContextStrategy {
    ContextStrategy::new_v2("terms", "Définitions choisies")
        .define_type("Term", term())
        .require(
            "terms",
            T::List {
                item: Box::new(T::Named {
                    name: "Term".into(),
                }),
            },
        )
        .with_program(vec![B::for_each(
            "each-term",
            E::resource("terms"),
            "term",
            vec![B::group(
                "term-group",
                "Terme",
                vec![
                    emit("name", E::field(E::variable("term"), "name")),
                    B::branch(
                        "has-definition",
                        P::negate(P::equal(
                            E::field(E::variable("term"), "definition"),
                            literal(""),
                        )),
                        vec![emit(
                            "definition",
                            E::truncate(E::field(E::variable("term"), "definition"), 4),
                        )],
                        vec![emit("fallback", literal("À définir"))],
                    ),
                ],
            )],
        )])
}

#[test]
fn foreach_has_distinct_ordered_occurrences_with_shared_source_values_and_branch_provenance() {
    let strategy = strategy();
    let source = generate(&strategy).unwrap();
    assert!(source.contains(".define_type(\"Term\""));
    assert!(source.contains("ContextBlock::for_each"));
    assert!(source.contains("ContextExpr::truncate"));
    assert_eq!(parse(&source).unwrap(), strategy);
    let resources = BTreeMap::from([(
        "terms".into(),
        Arc::new(json!([
            {"name":"Contexte","definition":"Été🌿 utile"}, {"name":"Flow","definition":""}
        ])),
    )]);
    let evaluation = evaluate(&strategy, &resources, &TypeRegistry::new());
    assert!(evaluation.complete, "{:?}", evaluation.diagnostics);
    let window = serde_json::to_value(&evaluation).unwrap();
    assert_eq!(window["items"][0]["items"][0]["value"], "Contexte");
    assert_eq!(window["items"][0]["items"][1]["value"], "Été🌿");
    assert_eq!(window["items"][1]["items"][0]["value"], "Flow");
    assert_eq!(window["items"][1]["items"][1]["value"], "À définir");
    let trace = &evaluation.trace;
    let conditions: Vec<_> = trace
        .iter()
        .filter(|entry| entry.block_id == "has-definition")
        .collect();
    assert_eq!(
        conditions
            .iter()
            .map(|entry| entry.outcome)
            .collect::<Vec<_>>(),
        vec![Some(true), Some(false)]
    );
    let names: Vec<_> = trace
        .iter()
        .filter(|entry| entry.block_id == "name")
        .collect();
    assert_ne!(names[0].id, names[1].id);
    assert_eq!(names[0].sources, ["terms"]);
    assert_eq!(names[1].iterations[0].index, 1);
    assert_eq!(window["items"][1]["items"][0]["sources"], json!(["terms"]));
    let repeat = evaluate(&strategy, &resources, &TypeRegistry::new());
    assert_eq!(serde_json::to_value(repeat).unwrap(), window);
    let zf_context::context::ContextItem::Group { items, .. } = &evaluation.items[0] else {
        panic!()
    };
    let zf_context::context::ContextItem::Fragment { value, .. } = &items[0] else {
        panic!()
    };
    let zf_context::context::ContextValue::Shared { root, pointer } = value else {
        panic!()
    };
    assert!(Arc::ptr_eq(root, &resources["terms"]));
    assert_eq!(pointer, "/0/name");
}

#[test]
fn nested_loop_variables_shadow_lexically_and_are_restored_without_leaking() {
    let list = T::List {
        item: Box::new(T::Text),
    };
    let strategy = ContextStrategy::new_v2("nested", "Nested")
        .require("outer", list.clone())
        .require("inner", list)
        .with_program(vec![B::for_each(
            "outer-loop",
            E::resource("outer"),
            "row",
            vec![
                emit("before", E::variable("row")),
                B::for_each(
                    "inner-loop",
                    E::resource("inner"),
                    "row",
                    vec![emit("inside", E::variable("row"))],
                ),
                emit("after", E::variable("row")),
            ],
        )]);
    let resources = BTreeMap::from([
        ("outer".into(), Arc::new(json!(["a", "b"]))),
        ("inner".into(), Arc::new(json!(["x", "y"]))),
    ]);
    let result = evaluate(&strategy, &resources, &TypeRegistry::new());
    assert!(result.complete, "{:?}", result.diagnostics);
    let window = serde_json::to_value(&result).unwrap();
    assert_eq!(
        window["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["value"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!("a"),
            json!("x"),
            json!("y"),
            json!("a"),
            json!("b"),
            json!("x"),
            json!("y"),
            json!("b")
        ]
    );
    assert_eq!(
        result
            .trace
            .iter()
            .find(|entry| entry.block_id == "inside")
            .unwrap()
            .iterations
            .len(),
        2
    );
    assert_eq!(window["items"][1]["sources"], json!(["inner"]));
    assert_eq!(window["items"][3]["sources"], json!(["outer"]));
    let ids: BTreeSet<_> = window["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 8);
    let mut leaking = strategy;
    leaking.program.push(emit("outside", E::variable("row")));
    assert!(
        validate_strategy(&leaking, &TypeRegistry::new())
            .unwrap_err()
            .iter()
            .any(|error| error.code == "unknown_variable")
    );
}

#[test]
fn missing_empty_wrong_types_and_nested_output_growth_are_not_approximated() {
    let strategy = strategy();
    let missing = evaluate(&strategy, &BTreeMap::new(), &TypeRegistry::new());
    assert!(!missing.complete);
    assert_eq!(missing.needs[0].resource, "terms");
    assert!(missing.items.is_empty());
    let empty = evaluate(
        &strategy,
        &BTreeMap::from([("terms".into(), Arc::new(json!([])))]),
        &TypeRegistry::new(),
    );
    assert!(empty.complete);
    assert!(empty.items.is_empty());
    let invalid = evaluate(
        &strategy,
        &BTreeMap::from([("terms".into(), Arc::new(json!("not a list")))]),
        &TypeRegistry::new(),
    );
    assert!(!invalid.complete);
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|error| error.code == "value_type")
    );
    let many = ContextStrategy::new_v2("bounded", "Bounded")
        .require(
            "rows",
            T::List {
                item: Box::new(T::Text),
            },
        )
        .with_program(vec![B::for_each(
            "outer",
            E::resource("rows"),
            "outer",
            vec![B::for_each(
                "inner",
                E::resource("rows"),
                "inner",
                vec![emit("row", E::variable("inner"))],
            )],
        )]);
    let bounded = evaluate_with_limits(
        &many,
        &BTreeMap::from([("rows".into(), Arc::new(json!(["a", "b", "c", "d"])))]),
        &TypeRegistry::new(),
        &ContextLibrary::default(),
        EvaluationLimits {
            max_steps: 1000,
            max_items: 8,
            max_bytes: 1000,
        },
    );
    assert!(!bounded.complete);
    assert!(bounded.items.len() <= 8);
    assert!(bounded.trace.len() <= 8);
    assert!(
        bounded
            .diagnostics
            .iter()
            .any(|error| error.code == "evaluation_limit")
    );
}

#[test]
fn text_truncation_is_explicit_unicode_safe_and_named_type_conflicts_are_rejected() {
    let policy = ContextStrategy::new_v2("text", "Text").with_program(vec![
        emit("zero", E::truncate(literal("🦀é"), 0)),
        emit("one", E::truncate(literal("🦀é"), 1)),
        emit("all", E::truncate(literal("🦀é"), 100)),
    ]);
    let result =
        serde_json::to_value(evaluate(&policy, &BTreeMap::new(), &TypeRegistry::new())).unwrap();
    assert_eq!(result["items"][0]["value"], "");
    assert_eq!(result["items"][1]["value"], "🦀");
    assert_eq!(result["items"][2]["value"], "🦀é");
    let invalid = ContextStrategy::new_v2("wrong", "Wrong").with_program(vec![emit(
        "bad",
        E::truncate(E::literal(T::Number, json!(42)), 2),
    )]);
    assert!(
        validate_strategy(&invalid, &TypeRegistry::new())
            .unwrap_err()
            .iter()
            .any(|error| error.code == "text_type")
    );
    assert!(
        validate_strategy(&strategy(), &BTreeMap::from([("Term".into(), T::Number)]))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "type_conflict")
    );
}

#[test]
fn constructed_lists_make_dynamic_message_parts_and_empty_lists_have_an_explicit_type() {
    let part_type = T::Record {
        fields: BTreeMap::from([("text".into(), T::Text)]),
    };
    let message_type = T::Record {
        fields: BTreeMap::from([
            ("role".into(), T::Text),
            (
                "parts".into(),
                T::List {
                    item: Box::new(part_type.clone()),
                },
            ),
        ]),
    };
    let parts = E::list(
        part_type,
        vec![E::record(BTreeMap::from([(
            "text".into(),
            E::resource("input"),
        )]))],
    );
    let messages = E::list(
        message_type,
        vec![E::record(BTreeMap::from([
            ("role".into(), literal("user")),
            ("parts".into(), parts),
        ]))],
    );
    let strategy = ContextStrategy::new_v2("messages", "Messages construits")
        .require("input", T::Text)
        .with_program(vec![
            B::emit("messages", R::Data, F::AdkMessages, messages),
            B::emit("empty", R::Data, F::Json, E::list(T::Number, vec![])),
        ]);
    let source = generate(&strategy).unwrap();
    assert_eq!(parse(&source).unwrap(), strategy);
    let evaluation = evaluate(
        &strategy,
        &BTreeMap::from([("input".into(), Arc::new(json!("Demande dynamique")))]),
        &TypeRegistry::new(),
    );
    assert!(evaluation.complete, "{:?}", evaluation.diagnostics);
    let result = serde_json::to_value(evaluation).unwrap();
    assert_eq!(
        result["items"][0]["value"],
        json!([{"role":"user","parts":[{"text":"Demande dynamique"}]}])
    );
    assert_eq!(result["items"][0]["sources"], json!(["input"]));
    assert_eq!(result["items"][1]["value"], json!([]));
    let invalid = ContextStrategy::new_v2("wrong-list", "Wrong").with_program(vec![B::emit(
        "invalid",
        R::Data,
        F::Json,
        E::list(T::Number, vec![literal("1")]),
    )]);
    assert!(
        validate_strategy(&invalid, &TypeRegistry::new())
            .unwrap_err()
            .iter()
            .any(|error| error.code == "list_item_type")
    );
}
