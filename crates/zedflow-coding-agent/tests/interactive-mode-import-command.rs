use zedflow_coding_agent::modes::interactive::get_path_command_argument;
#[test]
fn import_requires_a_complete_command_and_path() {
    assert_eq!(
        get_path_command_argument("/import archive.json", "/import"),
        Some("archive.json".into())
    );
    assert_eq!(get_path_command_argument("/important x", "/import"), None);
}
