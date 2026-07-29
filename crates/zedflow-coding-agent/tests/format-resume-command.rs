use zedflow_coding_agent::modes::interactive::{get_path_command_argument, quote_if_needed};
#[test]
fn commands_parse_paths_and_quote_spaces() {
    assert_eq!(
        get_path_command_argument("/import 'a b'", "/import"),
        Some("a b".into())
    );
    assert_eq!(quote_if_needed("a b"), "'a b'");
}
