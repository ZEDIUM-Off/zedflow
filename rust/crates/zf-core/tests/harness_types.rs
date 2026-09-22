use serde_json::json;
use zf_core::types::DataType;
use zf_core::types::TypeRegistry;
use zf_core::types::compatible;
use zf_core::types::validate_type;
use zf_core::types::validate_value;

#[test]
fn business_types_are_nominal_and_values_are_not_coerced() {
    let fields = [("name".into(), DataType::Text)].into();
    let types: TypeRegistry = [
        ("Term".into(), DataType::Record { fields }),
        (
            "Person".into(),
            DataType::Record {
                fields: [("name".into(), DataType::Text)].into(),
            },
        ),
    ]
    .into();
    let term = DataType::Named {
        name: "Term".into(),
    };
    assert!(compatible(&term, &term, &types));
    assert!(!compatible(
        &term,
        &DataType::Named {
            name: "Person".into()
        },
        &types
    ));
    assert!(validate_value(&term, &json!({"name":"Scope","source":"terms.md"}), &types).is_ok());
    assert_eq!(
        validate_value(&term, &json!({"name":42}), &types).unwrap_err()[0].path,
        "$.name"
    );
    assert!(validate_value(&DataType::Number, &json!("42"), &types).is_err());
    assert!(validate_value(&term, &json!({}), &types).is_err());
}

#[test]
fn missing_recursive_and_excessively_nested_types_produce_diagnostics() {
    let types: TypeRegistry = [
        ("A".into(), DataType::Named { name: "B".into() }),
        ("B".into(), DataType::Named { name: "A".into() }),
    ]
    .into();
    let a = DataType::Named { name: "A".into() };
    assert_eq!(validate_type(&a, &types).unwrap_err()[0].code, "type_cycle");
    assert!(!compatible(&a, &a, &types));
    assert_eq!(
        validate_type(
            &DataType::Named {
                name: "Missing".into()
            },
            &types
        )
        .unwrap_err()[0]
            .code,
        "unknown_type"
    );
    let mut nested = DataType::Text;
    for _ in 0..66 {
        nested = DataType::List {
            item: Box::new(nested),
        };
    }
    assert_eq!(
        validate_type(&nested, &types).unwrap_err()[0].code,
        "type_depth"
    );
}

#[test]
fn structural_records_and_media_references_preserve_their_contract() {
    let types = TypeRegistry::new();
    let small = DataType::Record {
        fields: [("label".into(), DataType::Text)].into(),
    };
    let large = DataType::Record {
        fields: [
            ("label".into(), DataType::Text),
            ("score".into(), DataType::Number),
        ]
        .into(),
    };
    assert!(compatible(&large, &small, &types));
    assert!(!compatible(&small, &large, &types));
    let image = DataType::Media {
        media_type: "image/png".into(),
    };
    assert!(
        validate_value(
            &image,
            &json!({"contentRef":"sha256:example","mediaType":"image/png"}),
            &types
        )
        .is_ok()
    );
    assert!(
        validate_value(
            &image,
            &json!({"contentRef":"sha256:example","mediaType":"audio/wav"}),
            &types
        )
        .is_err()
    );
    assert_eq!(
        serde_json::to_value(image).unwrap(),
        json!({"kind":"media","mediaType":"image/png"})
    );
}

#[test]
fn small_named_type_dags_cannot_expand_validation_exponentially() {
    let mut types = TypeRegistry::from([("T0".into(), DataType::Text)]);
    for index in 1..=30 {
        let child = DataType::Named {
            name: format!("T{}", index - 1),
        };
        types.insert(
            format!("T{index}"),
            DataType::Record {
                fields: [("left".into(), child.clone()), ("right".into(), child)].into(),
            },
        );
    }
    let errors = validate_type(&DataType::Named { name: "T30".into() }, &types).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "type_complexity");
}
