use zedflow_coding_agent::package_manager_cli::{
    PackageCommand, UpdateTarget, parse_package_command, update_target_includes_extensions,
    update_target_includes_self,
};

fn parse(args: &[&str]) -> zedflow_coding_agent::package_manager_cli::PackageCommandOptions {
    parse_package_command(&args.iter().map(ToString::to_string).collect::<Vec<_>>()).unwrap()
}

#[test]
fn update_defaults_to_self_and_preserves_extension_path_forms() {
    let default_update = parse(&["update"]);
    assert_eq!(default_update.update_target, Some(UpdateTarget::SelfOnly));
    assert!(default_update.show_extensions_skipped_note);

    let extension = parse(&["update", "--extension", "path:example"]);
    assert_eq!(
        extension.update_target,
        Some(UpdateTarget::Extensions(Some("path:example".into())))
    );
    assert!(update_target_includes_extensions(
        extension.update_target.as_ref().unwrap()
    ));
    assert!(!update_target_includes_self(
        extension.update_target.as_ref().unwrap()
    ));
}

#[test]
fn aliases_and_conflicts_match_pi_command_rules() {
    let uninstall = parse(&["uninstall", "path:example", "--local"]);
    assert_eq!(uninstall.command, PackageCommand::Remove);
    assert!(uninstall.local);

    let conflict = parse(&["update", "--all", "path:example"]);
    assert_eq!(
        conflict.conflicting_options.as_deref(),
        Some("--all cannot be combined with a positional source")
    );
}
