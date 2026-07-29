use std::{collections::HashMap, sync::Arc};

use serde_json::Value;

use super::super::{skills::Skill, source_info::SourceInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionError {
    pub message: String,
    pub source: Option<SourceInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionMode {
    Tui,
    Rpc,
    Json,
    Print,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub prompt_guidelines: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCommand {
    pub name: String,
    pub description: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionFlag {
    pub name: String,
    pub description: String,
    pub default: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionShortcut {
    pub key: String,
    pub command: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderConfig {
    pub name: String,
    pub config: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    pub name: String,
    pub source: SourceInfo,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineExtension {
    pub name: String,
    pub source: SourceInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExtensionEventKind {
    ProjectTrust,
    ResourcesDiscover,
    SessionStart,
    SessionInfoChanged,
    SessionBeforeSwitch,
    SessionBeforeFork,
    SessionBeforeCompact,
    SessionCompact,
    SessionShutdown,
    SessionBeforeTree,
    SessionTree,
    Context,
    BeforeProviderRequest,
    BeforeProviderHeaders,
    AfterProviderResponse,
    BeforeAgentStart,
    AgentStart,
    AgentEnd,
    TurnStart,
    TurnEnd,
    MessageStart,
    MessageUpdate,
    MessageEnd,
    ToolExecutionStart,
    ToolExecutionUpdate,
    ToolExecutionEnd,
    ModelSelect,
    ThinkingLevelSelect,
    UserBash,
    Input,
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionEvent {
    pub kind: ExtensionEventKind,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectTrustEventDecision {
    Yes,
    No,
    Undecided,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrustEvent {
    pub cwd: String,
    pub extensions: Vec<Extension>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrustEventResult {
    pub decision: ProjectTrustEventDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(String),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputEventResult {
    pub consumed: bool,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionActionResult {
    pub cancelled: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextUsage {
    pub tokens: Option<usize>,
    pub context_window: usize,
}

/// The host-provided, generation-scoped context visible to in-process handlers.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionContext {
    pub mode: ExtensionMode,
    pub cwd: String,
    pub has_ui: bool,
    pub generation: u64,
    pub stale: bool,
    pub model: Option<String>,
    pub context_usage: Option<ContextUsage>,
}

impl ExtensionContext {
    pub fn assert_active(&self) -> Result<(), ExtensionError> {
        if self.stale {
            Err(ExtensionError {
                message: "extension context is stale".into(),
                source: None,
            })
        } else {
            Ok(())
        }
    }
}

pub type ExtensionHandler = Arc<
    dyn Fn(&ExtensionEvent, &mut ExtensionContext) -> Result<Option<Value>, ExtensionError>
        + Send
        + Sync,
>;
pub type ToolHandler =
    Arc<dyn Fn(Value, &mut ExtensionContext) -> Result<Value, ExtensionError> + Send + Sync>;
pub type CommandHandler = Arc<
    dyn Fn(&[String], &mut ExtensionContext) -> Result<SessionActionResult, ExtensionError>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct RegisteredTool {
    pub info: ToolInfo,
    pub handler: ToolHandler,
}
#[derive(Clone)]
pub struct RegisteredExtensionCommand {
    pub info: RegisteredCommand,
    pub handler: CommandHandler,
}

#[derive(Clone, Default)]
pub struct ExtensionRuntime {
    pub tools: Vec<ToolInfo>,
    pub commands: Vec<RegisteredCommand>,
    pub flags: HashMap<String, bool>,
    pub shortcuts: Vec<ExtensionShortcut>,
    pub providers: Vec<ProviderConfig>,
    pub registered_tools: Vec<RegisteredTool>,
    pub registered_commands: Vec<RegisteredExtensionCommand>,
}

impl ExtensionRuntime {
    pub fn register_tool(&mut self, info: ToolInfo, handler: ToolHandler) {
        self.tools.push(info.clone());
        self.registered_tools.push(RegisteredTool { info, handler });
    }
    pub fn register_command(&mut self, info: RegisteredCommand, handler: CommandHandler) {
        self.commands.push(info.clone());
        self.registered_commands
            .push(RegisteredExtensionCommand { info, handler });
    }
    pub fn register_flag(&mut self, flag: ExtensionFlag) {
        self.flags.entry(flag.name).or_insert(flag.default);
    }
    pub fn register_shortcut(&mut self, shortcut: ExtensionShortcut) {
        self.shortcuts.push(shortcut);
    }
    pub fn register_provider(&mut self, provider: ProviderConfig) {
        self.providers.push(provider);
    }
}

#[derive(Debug, Clone, Default)]
pub struct LoadExtensionsResult {
    pub extensions: Vec<Extension>,
    pub errors: Vec<ExtensionError>,
}
pub type ExtensionFactory =
    Arc<dyn Fn(&mut ExtensionRuntime) -> Result<(), ExtensionError> + Send + Sync>;
pub type ExtensionErrorListener = Arc<dyn Fn(ExtensionError) + Send + Sync>;
pub type SkillResource = Skill;

#[must_use]
pub fn define_tool(name: impl Into<String>, description: impl Into<String>) -> ToolInfo {
    ToolInfo {
        name: name.into(),
        description: description.into(),
        parameters: Value::Null,
        prompt_guidelines: None,
    }
}
