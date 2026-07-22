use zedflow_coding_agent::{
    slash_commands::BUILTIN_SLASH_COMMANDS,
    source_info::{
        PathMetadata, SourceOrigin, SourceScope, create_source_info, create_synthetic_source_info,
    },
};

#[test]
fn source_info_preserves_metadata_and_synthetic_defaults() {
    let info = create_source_info(
        "skill.md",
        PathMetadata {
            source: "package-name".into(),
            scope: SourceScope::Project,
            origin: SourceOrigin::Package,
            base_dir: Some("/package".into()),
        },
    );
    assert_eq!(info.path, "skill.md");
    assert_eq!(info.source, "package-name");
    assert_eq!(info.scope, SourceScope::Project);
    assert_eq!(info.origin, SourceOrigin::Package);
    assert_eq!(info.base_dir.as_deref(), Some("/package"));

    let synthetic = create_synthetic_source_info("<builtin:test>", "builtin", None, None, None);
    assert_eq!(synthetic.scope, SourceScope::Temporary);
    assert_eq!(synthetic.origin, SourceOrigin::TopLevel);
    assert_eq!(synthetic.base_dir, None);
}

#[test]
fn builtin_slash_command_metadata_matches_pi() {
    assert_eq!(BUILTIN_SLASH_COMMANDS.len(), 22);
    assert_eq!(BUILTIN_SLASH_COMMANDS[0].name, "settings");
    assert_eq!(BUILTIN_SLASH_COMMANDS[21].name, "quit");
    assert_eq!(BUILTIN_SLASH_COMMANDS[21].description, "Quit pi");
    assert!(BUILTIN_SLASH_COMMANDS.iter().any(|command| {
        command.name == "reload"
            && command.description == "Reload keybindings, extensions, skills, prompts, and themes"
    }));
}
