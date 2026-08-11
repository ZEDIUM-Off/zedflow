use crate::source_info::SourceInfo;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlashCommandSource {
    Extension,
    Prompt,
    Skill,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlashCommandInfo {
    pub name: String,
    pub description: Option<String>,
    pub source: SlashCommandSource,
    pub source_info: SourceInfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuiltinSlashCommand {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinSlashCommandId {
    Settings,
    Model,
    ScopedModels,
    Export,
    Import,
    Share,
    Copy,
    Name,
    Session,
    Changelog,
    Hotkeys,
    Fork,
    Clone,
    Tree,
    Trust,
    Login,
    Logout,
    New,
    Compact,
    Resume,
    Reload,
    Quit,
}

impl BuiltinSlashCommandId {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Model => "model",
            Self::ScopedModels => "scoped-models",
            Self::Export => "export",
            Self::Import => "import",
            Self::Share => "share",
            Self::Copy => "copy",
            Self::Name => "name",
            Self::Session => "session",
            Self::Changelog => "changelog",
            Self::Hotkeys => "hotkeys",
            Self::Fork => "fork",
            Self::Clone => "clone",
            Self::Tree => "tree",
            Self::Trust => "trust",
            Self::Login => "login",
            Self::Logout => "logout",
            Self::New => "new",
            Self::Compact => "compact",
            Self::Resume => "resume",
            Self::Reload => "reload",
            Self::Quit => "quit",
        }
    }

    const fn accepts_arguments(self) -> bool {
        matches!(
            self,
            Self::Model | Self::Export | Self::Import | Self::Name | Self::Compact
        )
    }
}

/// Parse only the command forms accepted by Pi. `/exit` is an unlisted alias
/// for `/quit`; unknown or malformed slash text remains available to prompts,
/// skills, and the model.
#[must_use]
pub fn parse_builtin_slash_command(text: &str) -> Option<(BuiltinSlashCommandId, &str)> {
    let body = text.strip_prefix('/')?;
    let split = body.find(char::is_whitespace).unwrap_or(body.len());
    let (name, rest) = body.split_at(split);
    let command = match name {
        "settings" => BuiltinSlashCommandId::Settings,
        "model" => BuiltinSlashCommandId::Model,
        "scoped-models" => BuiltinSlashCommandId::ScopedModels,
        "export" => BuiltinSlashCommandId::Export,
        "import" => BuiltinSlashCommandId::Import,
        "share" => BuiltinSlashCommandId::Share,
        "copy" => BuiltinSlashCommandId::Copy,
        "name" => BuiltinSlashCommandId::Name,
        "session" => BuiltinSlashCommandId::Session,
        "changelog" => BuiltinSlashCommandId::Changelog,
        "hotkeys" => BuiltinSlashCommandId::Hotkeys,
        "fork" => BuiltinSlashCommandId::Fork,
        "clone" => BuiltinSlashCommandId::Clone,
        "tree" => BuiltinSlashCommandId::Tree,
        "trust" => BuiltinSlashCommandId::Trust,
        "login" => BuiltinSlashCommandId::Login,
        "logout" => BuiltinSlashCommandId::Logout,
        "new" => BuiltinSlashCommandId::New,
        "compact" => BuiltinSlashCommandId::Compact,
        "resume" => BuiltinSlashCommandId::Resume,
        "reload" => BuiltinSlashCommandId::Reload,
        "quit" | "exit" => BuiltinSlashCommandId::Quit,
        _ => return None,
    };
    let arguments = rest.trim_start();
    (arguments.is_empty() || command.accepts_arguments()).then_some((command, arguments))
}

pub const BUILTIN_SLASH_COMMANDS: &[BuiltinSlashCommand] = &[
    BuiltinSlashCommand {
        name: "settings",
        description: "Open settings menu",
    },
    BuiltinSlashCommand {
        name: "model",
        description: "Select model (opens selector UI)",
    },
    BuiltinSlashCommand {
        name: "scoped-models",
        description: "Enable/disable models for Ctrl+P cycling",
    },
    BuiltinSlashCommand {
        name: "export",
        description: "Export session (HTML default, or specify path: .html/.jsonl)",
    },
    BuiltinSlashCommand {
        name: "import",
        description: "Import and resume a session from a JSONL file",
    },
    BuiltinSlashCommand {
        name: "share",
        description: "Share session as a secret GitHub gist",
    },
    BuiltinSlashCommand {
        name: "copy",
        description: "Copy last agent message to clipboard",
    },
    BuiltinSlashCommand {
        name: "name",
        description: "Set session display name",
    },
    BuiltinSlashCommand {
        name: "session",
        description: "Show session info and stats",
    },
    BuiltinSlashCommand {
        name: "changelog",
        description: "Show changelog entries",
    },
    BuiltinSlashCommand {
        name: "hotkeys",
        description: "Show all keyboard shortcuts",
    },
    BuiltinSlashCommand {
        name: "fork",
        description: "Create a new fork from a previous user message",
    },
    BuiltinSlashCommand {
        name: "clone",
        description: "Duplicate the current session at the current position",
    },
    BuiltinSlashCommand {
        name: "tree",
        description: "Navigate session tree (switch branches)",
    },
    BuiltinSlashCommand {
        name: "trust",
        description: "Save project trust decision for future sessions",
    },
    BuiltinSlashCommand {
        name: "login",
        description: "Configure provider authentication",
    },
    BuiltinSlashCommand {
        name: "logout",
        description: "Remove provider authentication",
    },
    BuiltinSlashCommand {
        name: "new",
        description: "Start a new session",
    },
    BuiltinSlashCommand {
        name: "compact",
        description: "Manually compact the session context",
    },
    BuiltinSlashCommand {
        name: "resume",
        description: "Resume a different session",
    },
    BuiltinSlashCommand {
        name: "reload",
        description: "Reload keybindings, extensions, skills, prompts, and themes",
    },
    BuiltinSlashCommand {
        name: "quit",
        description: "Quit pi",
    },
];
