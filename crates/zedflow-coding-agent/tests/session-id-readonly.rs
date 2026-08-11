use zedflow_coding_agent::parse_args;

#[test]
fn read_only_commands_keep_session_id_as_data_without_creating_a_session() {
    for args in [
        vec!["--session-id", "read-only-help", "--help"],
        vec!["--no-session", "--session-id", "ephemeral-id", "--help"],
        vec!["--session-id", "read-only-models", "--list-models"],
    ] {
        let parsed = parse_args(args);
        assert!(parsed.help || parsed.list_models.is_some());
        assert!(parsed.session_id.is_some());
    }
}
