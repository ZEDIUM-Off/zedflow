use zedflow_ai::types::Model;
use zedflow_coding_agent::model_resolver::{find_exact_model_reference_match, parse_model_pattern};

#[test]
fn model_patterns_resolve_provider_qualified_references() {
    let models = vec![Model {
        provider: "anthropic".into(),
        id: "claude-test".into(),
        name: "Claude Test".into(),
        ..Model::default()
    }];
    assert_eq!(
        find_exact_model_reference_match("anthropic/claude-test", &models)
            .unwrap()
            .id,
        "claude-test"
    );
    assert_eq!(
        parse_model_pattern("claude-test", &models, true)
            .model
            .unwrap()
            .provider,
        "anthropic"
    );
}
