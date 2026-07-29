use zedflow_coding_agent::modes::interactive::is_anthropic_subscription_auth_key;
#[test]
fn anthropic_subscription_keys_are_detected() {
    assert!(is_anthropic_subscription_auth_key(Some("sk-ant-oat-token")));
    assert!(!is_anthropic_subscription_auth_key(Some("sk-other")));
}
