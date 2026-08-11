use zedflow_coding_agent::package_manager_cli::{
    PackageCommand, UpdateTarget, parse_package_command,
};
#[test]
fn update_command_selects_extension_target() {
    let command = parse_package_command(&["update".into(), "--extensions".into()]).unwrap();
    assert_eq!(command.command, PackageCommand::Update);
    assert_eq!(command.update_target, Some(UpdateTarget::Extensions(None)));
}
