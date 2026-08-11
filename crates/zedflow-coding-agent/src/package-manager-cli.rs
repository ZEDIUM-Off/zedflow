//! Parsing for Pi-compatible package command paths.
//!
//! Execution is intentionally limited to the source-only native package manager;
//! this parser preserves Pi's command/option selection without running shell or
//! TypeScript package installers.

pub const MODULE_PATH: &str = "package-manager-cli.rs";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageCommand {
    Install,
    Remove,
    Update,
    List,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateTarget {
    All,
    SelfOnly,
    Extensions(Option<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCommandOptions {
    pub command: PackageCommand,
    pub source: Option<String>,
    pub update_target: Option<UpdateTarget>,
    pub show_extensions_skipped_note: bool,
    pub local: bool,
    pub force: bool,
    pub project_trust_override: Option<bool>,
    pub help: bool,
    pub invalid_option: Option<String>,
    pub invalid_argument: Option<String>,
    pub missing_option_value: Option<String>,
    pub conflicting_options: Option<String>,
}

/// Parses one of Pi's install/remove/update/list command forms. `uninstall` is
/// retained as an alias for remove.
#[must_use]
pub fn parse_package_command(args: &[String]) -> Option<PackageCommandOptions> {
    let (raw, rest) = args.split_first()?;
    let command = match raw.as_str() {
        "install" => PackageCommand::Install,
        "remove" | "uninstall" => PackageCommand::Remove,
        "update" => PackageCommand::Update,
        "list" => PackageCommand::List,
        _ => return None,
    };
    let mut local = false;
    let mut force = false;
    let mut project_trust_override = None;
    let mut help = false;
    let mut invalid_option = None;
    let mut invalid_argument = None;
    let mut missing_option_value = None;
    let mut conflicting_options = None;
    let mut source = None;
    let mut self_flag = false;
    let mut extensions_flag = false;
    let mut all_flag = false;
    let mut extension_source = None;
    let mut index = 0;
    while index < rest.len() {
        let arg = &rest[index];
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "-l" | "--local"
                if matches!(command, PackageCommand::Install | PackageCommand::Remove) =>
            {
                local = true
            }
            "-l" | "--local" => set_once(&mut invalid_option, arg),
            "--self" if command == PackageCommand::Update => self_flag = true,
            "--extensions" if command == PackageCommand::Update => extensions_flag = true,
            "--all" if command == PackageCommand::Update => all_flag = true,
            "--self" | "--extensions" | "--all" => set_once(&mut invalid_option, arg),
            "-a" | "--approve" => project_trust_override = Some(true),
            "-na" | "--no-approve" => project_trust_override = Some(false),
            "--force" if command == PackageCommand::Update => force = true,
            "--force" => set_once(&mut invalid_option, arg),
            "--extension" if command == PackageCommand::Update => {
                let value = rest.get(index + 1).filter(|value| !value.starts_with('-'));
                match (value, extension_source.is_some()) {
                    (None, _) => set_once(&mut missing_option_value, arg),
                    (Some(_), true) => {
                        set_once(
                            &mut conflicting_options,
                            "--extension can only be provided once",
                        );
                        index += 1;
                    }
                    (Some(value), false) => {
                        extension_source = Some(value.clone());
                        index += 1;
                    }
                }
            }
            "--extension" => set_once(&mut invalid_option, arg),
            _ if arg.starts_with('-') => set_once(&mut invalid_option, arg),
            _ if source.is_none() => source = Some(arg.clone()),
            _ => set_once(&mut invalid_argument, arg),
        }
        index += 1;
    }
    let (update_target, show_extensions_skipped_note) = if command != PackageCommand::Update {
        (None, false)
    } else {
        if all_flag && (self_flag || extensions_flag || extension_source.is_some()) {
            set_once(
                &mut conflicting_options,
                "--all cannot be combined with --self, --extensions, or --extension",
            );
        }
        if all_flag && source.is_some() {
            set_once(
                &mut conflicting_options,
                "--all cannot be combined with a positional source",
            );
        }
        let target = if let Some(extension) = extension_source {
            if self_flag || extensions_flag || all_flag {
                set_once(
                    &mut conflicting_options,
                    "--extension cannot be combined with --self, --extensions, or --all",
                );
            }
            if source.is_some() {
                set_once(
                    &mut conflicting_options,
                    "--extension cannot be combined with a positional source",
                );
            }
            UpdateTarget::Extensions(Some(extension))
        } else if let Some(value) = &source {
            if value == "self" || value == "pi" {
                if extensions_flag {
                    UpdateTarget::All
                } else {
                    UpdateTarget::SelfOnly
                }
            } else {
                if extensions_flag || self_flag || all_flag {
                    set_once(
                        &mut conflicting_options,
                        "positional update targets cannot be combined with --self, --extensions, or --all",
                    );
                }
                UpdateTarget::Extensions(Some(value.clone()))
            }
        } else if all_flag || (self_flag && extensions_flag) {
            UpdateTarget::All
        } else if extensions_flag {
            UpdateTarget::Extensions(None)
        } else {
            UpdateTarget::SelfOnly
        };
        let skipped = source.is_none()
            && !all_flag
            && !self_flag
            && !extensions_flag
            && target == UpdateTarget::SelfOnly;
        (Some(target), skipped)
    };
    Some(PackageCommandOptions {
        command,
        source,
        update_target,
        show_extensions_skipped_note,
        local,
        force,
        project_trust_override,
        help,
        invalid_option,
        invalid_argument,
        missing_option_value,
        conflicting_options,
    })
}

fn set_once(slot: &mut Option<String>, value: impl Into<String>) {
    if slot.is_none() {
        *slot = Some(value.into());
    }
}

#[must_use]
pub fn update_target_includes_self(target: &UpdateTarget) -> bool {
    matches!(target, UpdateTarget::All | UpdateTarget::SelfOnly)
}

#[must_use]
pub fn update_target_includes_extensions(target: &UpdateTarget) -> bool {
    matches!(target, UpdateTarget::All | UpdateTarget::Extensions(_))
}
