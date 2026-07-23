use std::collections::HashSet;

use zedflow_coding_agent::modes::interactive::{
    InteractiveMode, get_path_command_argument, is_anthropic_subscription_auth_key,
    is_api_key_login_provider, quote_if_needed,
};

#[test]
fn path_commands_follow_pi_argument_rules() {
    assert_eq!(
        get_path_command_argument("/import 'path with spaces/session.jsonl'", "/import"),
        Some("path with spaces/session.jsonl".into())
    );
    assert_eq!(
        get_path_command_argument("/import john's/session.jsonl", "/import"),
        Some("john's/session.jsonl".into())
    );
    assert_eq!(
        get_path_command_argument("/important /tmp/session.jsonl", "/import"),
        None
    );
}

#[test]
fn startup_input_is_trimmed_and_consumed_in_order() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("  first  ");
    mode.queue_user_input("   ");
    mode.queue_user_input("second");

    assert_eq!(mode.pending_user_input_count(), 2);
    assert_eq!(mode.get_user_input().as_deref(), Some("first"));
    assert_eq!(mode.get_user_input().as_deref(), Some("second"));
    assert_eq!(mode.get_user_input(), None);
}

#[test]
fn provider_login_and_auth_key_rules_match_pi() {
    let oauth = HashSet::from(["oauth-provider".to_owned()]);
    let builtins = HashSet::from(["builtin-provider".to_owned()]);
    let api_key_builtins = HashSet::from(["openai".to_owned()]);

    assert!(is_api_key_login_provider(
        "openai",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(!is_api_key_login_provider(
        "builtin-provider",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(!is_api_key_login_provider(
        "oauth-provider",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(is_anthropic_subscription_auth_key(Some(
        "sk-ant-oat01-test"
    )));
    assert!(!is_anthropic_subscription_auth_key(Some("sk-ant-api-test")));
}

#[test]
fn shell_quote_only_when_needed() {
    assert_eq!(quote_if_needed("/tmp/session.jsonl"), "/tmp/session.jsonl");
    assert_eq!(quote_if_needed("path with spaces"), "'path with spaces'");
    assert_eq!(quote_if_needed("john's"), "'john'\\''s'");
}
