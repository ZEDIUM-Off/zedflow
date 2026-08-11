use zedflow_coding_agent::experimental::are_experimental_features_enabled;

#[test]
fn experimental_gate_requires_the_literal_one_value() {
    assert_eq!(
        are_experimental_features_enabled(),
        std::env::var_os("PI_EXPERIMENTAL").is_some_and(|value| value == "1")
    );
}
