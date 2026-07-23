use std::{collections::HashMap, sync::Arc};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCommand {
    pub name: String,
    pub description: String,
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
#[derive(Debug, Clone, Default)]
pub struct ExtensionRuntime {
    pub tools: Vec<ToolInfo>,
    pub commands: Vec<RegisteredCommand>,
    pub flags: HashMap<String, bool>,
}
#[derive(Debug, Clone, Default)]
pub struct LoadExtensionsResult {
    pub extensions: Vec<Extension>,
    pub errors: Vec<ExtensionError>,
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
pub type ExtensionFactory =
    Arc<dyn Fn(&mut ExtensionRuntime) -> Result<(), ExtensionError> + Send + Sync>;
pub type ExtensionErrorListener = Arc<dyn Fn(ExtensionError) + Send + Sync>;
pub type SkillResource = Skill;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    Key(String),
    Text(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputEventResult {
    pub consumed: bool,
}

#[must_use]
pub fn define_tool(name: impl Into<String>, description: impl Into<String>) -> ToolInfo {
    ToolInfo {
        name: name.into(),
        description: description.into(),
    }
}
