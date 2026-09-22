use std::collections::BTreeMap;
use zf_context::context::ContextStrategy;
use zf_context::context::validate_strategy;
use zf_context::context_json;
use zf_context::context_source;
use zf_core::types::DataType;
use zf_core::types::TypeRegistry;

fn nested_type(depth: usize) -> DataType {
    let mut data_type = DataType::Text;
    for index in (1..=depth).rev() {
        data_type = DataType::Record {
            fields: BTreeMap::from([(format!("level_{index}"), data_type)]),
        };
    }
    data_type
}

#[test]
fn supported_context_depth_survives_json_rust_and_validation() {
    for depth in [32, 64] {
        let strategy =
            ContextStrategy::new_v2("deep-codec", "Deep codec").require("deep", nested_type(depth));
        let json = serde_json::to_vec(&strategy).unwrap();
        let restored: ContextStrategy = context_json::from_slice(&json).unwrap();
        assert_eq!(strategy, restored);
        validate_strategy(&strategy, &TypeRegistry::new()).unwrap();
        let source = context_source::generate(&strategy).unwrap();
        assert_eq!(context_source::parse(&source).unwrap(), strategy);
    }
}
