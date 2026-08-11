use zedflow_coding_agent::modes::interactive::{InteractiveMode, quote_if_needed};

#[test]
fn clone_paths_are_shell_quoted_when_needed() {
    assert_eq!(quote_if_needed("repo name"), "'repo name'");
    assert_eq!(quote_if_needed("repo/name"), "repo/name");
}

#[test]
fn clone_is_dispatched_without_prompting() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/clone");
    assert_eq!(mode.last_status(), Some("Clone requested"));
    assert_eq!(mode.pending_user_input_count(), 0);
}

#[test]
fn malformed_clone_text_preserves_unknown_slash_behavior() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/clone elsewhere");
    assert_eq!(mode.get_user_input().as_deref(), Some("/clone elsewhere"));
}
