use zedflow_coding_agent::{
    modes::interactive::{InteractiveMode, get_path_command_argument},
    slash_commands::{BuiltinSlashCommandId, parse_builtin_slash_command},
};

#[test]
fn import_requires_a_complete_command_and_path() {
    assert_eq!(
        get_path_command_argument("/import archive.json", "/import"),
        Some("archive.json".into())
    );
    assert_eq!(get_path_command_argument("/important x", "/import"), None);
    assert_eq!(
        parse_builtin_slash_command("/import archive.json"),
        Some((BuiltinSlashCommandId::Import, "archive.json"))
    );
}

#[test]
fn missing_import_path_is_an_interactive_error_not_a_prompt() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/import");
    assert_eq!(mode.last_status(), Some("Usage: /import <path.jsonl>"));
    assert_eq!(mode.pending_user_input_count(), 0);
}

#[test]
fn import_path_opens_confirmation_without_prompting() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/import 'archive file.jsonl'");
    assert_eq!(
        mode.last_status(),
        Some("Import confirmation opened for 'archive file.jsonl'")
    );
    assert_eq!(mode.pending_user_input_count(), 0);
}
