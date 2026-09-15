//! Pure capture, validation and edits of an immutable prepared window.
//! Persistence and compare-and-swap publication belong to storage adapters.
use super::context::{
    ContextCapability, ContextEvaluation, ContextStrategy, FragmentFormat, FragmentRole,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zf_core::{
    diagnostics::Diagnostic,
    identity::{Permission, Scope},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowGrant {
    pub alias: String,
    pub permission: Permission,
}

pub fn grants(config: &Value) -> anyhow::Result<Vec<WindowGrant>> {
    let grants: Vec<WindowGrant> = serde_json::from_value(
        config
            .get("windowGrants")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
    )?;
    let mut names = BTreeSet::new();
    for grant in &grants {
        anyhow::ensure!(
            !grant.alias.trim().is_empty() && names.insert(&grant.alias),
            "Window grants require unique nonempty aliases"
        );
    }
    Ok(grants)
}

pub fn is_tool(name: &str) -> bool {
    matches!(name, "context_window_read" | "context_window_patch")
}

pub fn capability(name: &str) -> Option<ContextCapability> {
    use zf_core::types::DataType;
    if !is_tool(name) {
        return None;
    }
    let mut input = BTreeMap::from([("alias".into(), DataType::Text)]);
    if name == "context_window_patch" {
        input.insert("expectedRevision".into(), DataType::Text);
        input.insert(
            "patches".into(),
            DataType::List {
                item: Box::new(DataType::Record {
                    fields: BTreeMap::new(),
                }),
            },
        );
    }
    let output = BTreeMap::from([
        ("entityId".into(), DataType::Text),
        ("revision".into(), DataType::Text),
        ("contentRef".into(), DataType::Text),
        (
            "window".into(),
            DataType::Record {
                fields: BTreeMap::new(),
            },
        ),
    ]);
    Some(ContextCapability::new(
        name,
        DataType::Record { fields: input },
        DataType::Record { fields: output },
    ))
}

pub fn declarations(config: &Value) -> anyhow::Result<Vec<Value>> {
    use serde_json::json;
    let grants = grants(config)?;
    if grants.is_empty() {
        return Ok(vec![]);
    }
    let read: Vec<_> = grants.iter().map(|g| g.alias.as_str()).collect();
    let write: Vec<_> = grants
        .iter()
        .filter(|g| g.permission == Permission::Write)
        .map(|g| g.alias.as_str())
        .collect();
    let mut tools = vec![
        json!({"name":"context_window_read","description":"Read a prepared context window through an explicitly granted local alias. Capture its revision before editing.","parameters":{"type":"object","properties":{"alias":{"type":"string","enum":read},"revision":{"type":"string"}},"required":["alias"],"additionalProperties":false}}),
    ];
    if !write.is_empty() {
        tools.push(json!({"name":"context_window_patch","description":"Publish explicit remove, replace, move or representation edits against the exact expected window revision. Does not modify an inference already using a captured revision.","parameters":{"type":"object","properties":{"alias":{"type":"string","enum":write},"expectedRevision":{"type":"string"},"patches":{"type":"array","minItems":1,"maxItems":MAX_ITEMS,"items":{"type":"object"}}},"required":["alias","expectedRevision","patches"],"additionalProperties":false}}));
    }
    Ok(tools)
}

/// Scope belongs to the caller's flow instance; neither tool arguments nor
/// authored grants can select another namespace. Sharing requires a bridge alias.
pub fn agent_scope(path: &str) -> Scope {
    Scope::Flow(
        path.rsplit_once('/')
            .map_or("root", |(instance, _)| instance)
            .into(),
    )
}

const MAX_ITEMS: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum WindowItem {
    Group {
        id: String,
        label: String,
        items: Vec<WindowItem>,
    },
    Fragment {
        id: String,
        role: FragmentRole,
        format: FragmentFormat,
        value: Value,
        sources: Vec<String>,
    },
}
impl WindowItem {
    pub fn id(&self) -> &str {
        match self {
            Self::Group { id, .. } | Self::Fragment { id, .. } => id,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedWindow {
    pub strategy_id: String,
    pub strategy_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    pub items: Vec<WindowItem>,
    pub source_revisions: BTreeMap<String, String>,
    pub capabilities: Vec<ContextCapability>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum WindowPatch {
    Remove {
        id: String,
    },
    Replace {
        id: String,
        item: WindowItem,
    },
    /// Destination index is interpreted after removing the item from its old parent.
    Move {
        id: String,
        parent: Option<String>,
        index: usize,
    },
    Representation {
        id: String,
        format: FragmentFormat,
        value: Value,
    },
}
#[derive(Debug)]
pub enum WindowError {
    Invalid(Vec<Diagnostic>),
}
impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(errors) => write!(
                f,
                "{}",
                errors
                    .iter()
                    .map(|e| format!("{}: {} ({})", e.path, e.message, e.code))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }
}
impl std::error::Error for WindowError {}
pub type Result<T> = std::result::Result<T, WindowError>;
fn invalid(code: &str, path: &str, message: &str) -> WindowError {
    WindowError::Invalid(vec![Diagnostic::new(code, path, message)])
}

pub fn capture(
    strategy: &ContextStrategy,
    strategy_revision: &str,
    evaluation: &ContextEvaluation,
    source_revisions: BTreeMap<String, String>,
) -> Result<PreparedWindow> {
    if !evaluation.complete || !evaluation.diagnostics.is_empty() || !evaluation.needs.is_empty() {
        return Err(invalid(
            "window_incomplete",
            "window",
            "Only a complete evaluation can become a prepared window",
        ));
    }
    for read in &evaluation.reads {
        if !source_revisions.contains_key(read) {
            return Err(invalid(
                "window_source_revision",
                read,
                "Every consulted source, including an absent source, needs a captured revision or explicit absence marker",
            ));
        }
    }
    let items = serde_json::from_value(serde_json::to_value(&evaluation.items).map_err(|_| {
        invalid(
            "window_value",
            "items",
            "Window values cannot be serialized",
        )
    })?)
    .map_err(|_| invalid("window_value", "items", "Prepared items are malformed"))?;
    let window = PreparedWindow {
        strategy_id: strategy.id.clone(),
        strategy_revision: strategy_revision.into(),
        program_revision: None,
        items,
        source_revisions,
        capabilities: evaluation.capabilities.clone(),
    };
    validate(&window)?;
    Ok(window)
}

pub fn validate(window: &PreparedWindow) -> Result<()> {
    if window.strategy_id.trim().is_empty() || window.strategy_revision.trim().is_empty() {
        return Err(invalid(
            "window_strategy",
            "window",
            "Window must identify its captured strategy revision",
        ));
    }
    if window
        .source_revisions
        .values()
        .any(|v| v.trim().is_empty())
    {
        return Err(invalid(
            "window_source_revision",
            "sourceRevisions",
            "Source revision identities cannot be empty",
        ));
    }
    let mut ids = BTreeSet::new();
    fn items(
        values: &[WindowItem],
        depth: usize,
        ids: &mut BTreeSet<String>,
        sources: &BTreeMap<String, String>,
    ) -> Result<()> {
        if depth > MAX_DEPTH {
            return Err(invalid(
                "window_limit",
                "items",
                "Window nesting exceeds 64 levels",
            ));
        }
        for item in values {
            if ids.len() >= MAX_ITEMS {
                return Err(invalid(
                    "window_limit",
                    "items",
                    "Window exceeds its item limit",
                ));
            }
            if item.id().trim().is_empty() || !ids.insert(item.id().to_owned()) {
                return Err(invalid(
                    "window_id",
                    item.id(),
                    "Window item identifiers must be nonempty and unique",
                ));
            }
            match item {
                WindowItem::Group {
                    label,
                    items: children,
                    ..
                } => {
                    if label.trim().is_empty() {
                        return Err(invalid(
                            "window_group",
                            item.id(),
                            "Group label cannot be empty",
                        ));
                    }
                    items(children, depth + 1, ids, sources)?;
                }
                WindowItem::Fragment {
                    id,
                    role,
                    format,
                    value,
                    sources: read,
                } => {
                    if *role == FragmentRole::Instruction && *format != FragmentFormat::Text {
                        return Err(invalid(
                            "window_format",
                            id,
                            "Instructions require a text representation",
                        ));
                    }
                    let correct = match format {
                        FragmentFormat::Text => value.is_string(),
                        FragmentFormat::Json => true,
                        FragmentFormat::AdkMessages => value
                            .as_array()
                            .is_some_and(|items| items.iter().all(Value::is_object)),
                        FragmentFormat::Media => {
                            value
                                .get("contentRef")
                                .and_then(Value::as_str)
                                .is_some_and(|v| !v.is_empty())
                                && value
                                    .get("mediaType")
                                    .and_then(Value::as_str)
                                    .is_some_and(|v| !v.is_empty())
                        }
                    };
                    if !correct {
                        return Err(invalid(
                            "window_format",
                            id,
                            "Value does not match its explicit representation",
                        ));
                    }
                    if read.iter().any(|name| !sources.contains_key(name)) {
                        return Err(invalid(
                            "window_source_revision",
                            id,
                            "Fragment source has no captured revision",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    items(&window.items, 0, &mut ids, &window.source_revisions)?;
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_BYTES.saturating_sub(self.0) {
                return Err(std::io::Error::other("window byte limit"));
            }
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(&mut Counter(0), window)
        .map_err(|_| invalid("window_limit", "window", "Window exceeds 16 MiB"))?;
    Ok(())
}

/// Compute a new immutable window. Failure cannot expose a partially applied patch.
pub fn patched(window: &PreparedWindow, patches: &[WindowPatch]) -> Result<PreparedWindow> {
    validate(window)?;
    let mut updated = window.clone();
    apply_patches(&mut updated, patches)?;
    Ok(updated)
}

fn apply_patches(window: &mut PreparedWindow, patches: &[WindowPatch]) -> Result<()> {
    if patches.is_empty() || patches.len() > MAX_ITEMS {
        return Err(invalid(
            "window_patch",
            "patches",
            "Submit 1–4096 explicit window edits",
        ));
    }
    for (index, patch) in patches.iter().enumerate() {
        let path = format!("patches[{index}]");
        match patch {
            WindowPatch::Remove { id } => {
                remove(&mut window.items, id)
                    .ok_or_else(|| invalid("window_item", &path, "Unknown window item"))?;
            }
            WindowPatch::Replace { id, item } => {
                if item.id() != id {
                    return Err(invalid(
                        "window_id",
                        &path,
                        "Replacement must preserve item identity",
                    ));
                }
                *find_mut(&mut window.items, id)
                    .ok_or_else(|| invalid("window_item", &path, "Unknown window item"))? =
                    item.clone();
            }
            WindowPatch::Representation { id, format, value } => {
                let WindowItem::Fragment {
                    format: current,
                    value: content,
                    ..
                } = find_mut(&mut window.items, id)
                    .ok_or_else(|| invalid("window_item", &path, "Unknown window item"))?
                else {
                    return Err(invalid(
                        "window_representation",
                        &path,
                        "Only fragments have a representation",
                    ));
                };
                *current = *format;
                *content = value.clone();
            }
            WindowPatch::Move { id, parent, index } => {
                let item = remove(&mut window.items, id)
                    .ok_or_else(|| invalid("window_item", &path, "Unknown window item"))?;
                // Removing first makes the item and all descendants unavailable
                // as destinations, so a move cannot create a parent cycle.
                let target = if let Some(parent) = parent {
                    let Some(WindowItem::Group { items, .. }) = find_mut(&mut window.items, parent)
                    else {
                        return Err(invalid(
                            "window_parent",
                            &path,
                            "Destination must be an existing group outside the moved subtree",
                        ));
                    };
                    items
                } else {
                    &mut window.items
                };
                if *index > target.len() {
                    return Err(invalid(
                        "window_index",
                        &path,
                        "Destination index is outside the group",
                    ));
                }
                target.insert(*index, item);
            }
        }
        validate(window)?;
    }
    Ok(())
}

pub fn decode(value: &Value) -> Result<PreparedWindow> {
    let window: PreparedWindow = PreparedWindow::deserialize(value).map_err(|_| {
        invalid(
            "window_value",
            "window",
            "Entity does not contain a prepared window",
        )
    })?;
    validate(&window)?;
    Ok(window)
}
fn find_mut<'a>(items: &'a mut [WindowItem], id: &str) -> Option<&'a mut WindowItem> {
    for item in items {
        if item.id() == id {
            return Some(item);
        }
        if let WindowItem::Group { items, .. } = item
            && let Some(item) = find_mut(items, id)
        {
            return Some(item);
        }
    }
    None
}
fn remove(items: &mut Vec<WindowItem>, id: &str) -> Option<WindowItem> {
    if let Some(index) = items.iter().position(|item| item.id() == id) {
        return Some(items.remove(index));
    }
    for item in items {
        if let WindowItem::Group { items, .. } = item
            && let Some(item) = remove(items, id)
        {
            return Some(item);
        }
    }
    None
}
