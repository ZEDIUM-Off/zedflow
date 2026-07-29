use zedflow_coding_agent::modes::interactive::quote_if_needed;
#[test]
fn clone_paths_are_shell_quoted_when_needed() {
    assert_eq!(quote_if_needed("repo name"), "'repo name'");
    assert_eq!(quote_if_needed("repo/name"), "repo/name");
}
