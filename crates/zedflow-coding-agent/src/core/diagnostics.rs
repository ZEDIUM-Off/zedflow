/// Kind of resource involved in a collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Extension,
    Skill,
    Prompt,
    Theme,
}

/// Two resources with the same name and the resolution winner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCollision {
    pub resource_type: ResourceType,
    pub name: String,
    pub winner_path: String,
    pub loser_path: String,
    pub winner_source: Option<String>,
    pub loser_source: Option<String>,
}

/// Severity or category of a resource-loading diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceDiagnosticType {
    Warning,
    Error,
    Collision,
}

/// Warning, error, or collision produced while loading resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDiagnostic {
    pub r#type: ResourceDiagnosticType,
    pub message: String,
    pub path: Option<String>,
    pub collision: Option<ResourceCollision>,
}
