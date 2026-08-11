//! Pi-compatible session contracts.
//!
//! Storage is implemented once in `zedflow-agent`; this module is the
//! coding-agent-facing namespace and keeps callers from depending on the
//! lower-level harness path.

use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
    time::SystemTime,
};

pub use zedflow_agent::harness::session::{
    InMemorySessionRepo, InMemorySessionStorage, InMemorySessionStorageOptions, JsonlSessionRepo,
    JsonlSessionStorage, JsonlSessionStorageCreateOptions, JsonlSessionStorageFileSystem,
    build_session_context, create_session_id, create_timestamp, get_entries_to_fork,
    load_jsonl_session_metadata, uuidv7,
};
pub use zedflow_agent::harness::types::{
    ActiveToolsChangeEntry, BranchSummaryEntry, CompactionEntry, CustomMessageEntry,
    JsonlSessionCreateOptions, JsonlSessionListOptions, JsonlSessionMetadata, LabelEntry,
    MessageEntry, ModelChangeEntry, SessionContext, SessionError, SessionErrorCode,
    SessionForkOptions, SessionForkPosition, SessionInfoEntry, SessionMetadata, SessionRepo,
    SessionStorage, SessionTreeEntry, SessionTreeEntryBase, ThinkingLevelChangeEntry,
};

/// Current session state used by the coding-agent layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub cwd: String,
    pub session_file: Option<String>,
    pub session_id: String,
    pub name: Option<String>,
    pub parent_session_path: Option<String>,
    pub created: Option<String>,
    pub modified: Option<SystemTime>,
    pub message_count: usize,
    pub first_message: String,
    pub all_messages_text: String,
}

impl SessionInfo {
    #[must_use]
    pub fn in_memory(cwd: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            session_file: None,
            session_id: session_id.into(),
            name: None,
            parent_session_path: None,
            created: None,
            modified: None,
            message_count: 0,
            first_message: "(no messages)".into(),
            all_messages_text: String::new(),
        }
    }

    #[must_use]
    pub fn persisted(
        cwd: impl Into<String>,
        file: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            session_file: Some(file.into()),
            ..Self::in_memory(cwd, session_id)
        }
    }

    #[must_use]
    pub fn is_persisted(&self) -> bool {
        self.session_file.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionTreeNode {
    pub entry: SessionTreeEntry,
    pub children: Vec<SessionTreeNode>,
    pub label: Option<String>,
    pub label_timestamp: Option<String>,
}

#[must_use]
pub fn build_session_tree(entries: &[SessionTreeEntry]) -> Vec<SessionTreeNode> {
    let mut labels: HashMap<&str, (&Option<String>, &str)> = HashMap::new();
    for entry in entries {
        if let SessionTreeEntry::Label(label) = entry {
            labels.insert(&label.target_id, (&label.label, &label.base.timestamp));
        }
    }
    let ids = entries
        .iter()
        .map(|entry| entry_base(entry).id.as_str())
        .collect::<HashSet<_>>();
    let mut children = HashMap::<Option<&str>, Vec<usize>>::new();
    for (index, entry) in entries.iter().enumerate() {
        let base = entry_base(entry);
        let parent = base
            .parent_id
            .as_deref()
            .filter(|parent| *parent != base.id && ids.contains(*parent));
        children.entry(parent).or_default().push(index);
    }

    fn node(
        index: usize,
        entries: &[SessionTreeEntry],
        children: &HashMap<Option<&str>, Vec<usize>>,
        labels: &HashMap<&str, (&Option<String>, &str)>,
    ) -> SessionTreeNode {
        let entry = entries[index].clone();
        let base = entry_base(&entry);
        let (label, label_timestamp) = labels
            .get(base.id.as_str())
            .map_or((None, None), |(label, timestamp)| {
                ((*label).clone(), Some((*timestamp).to_owned()))
            });
        let mut nested = children
            .get(&Some(base.id.as_str()))
            .into_iter()
            .flatten()
            .map(|child| node(*child, entries, children, labels))
            .collect::<Vec<_>>();
        nested.sort_by(|left, right| {
            entry_base(&left.entry)
                .timestamp
                .cmp(&entry_base(&right.entry).timestamp)
        });
        SessionTreeNode {
            entry,
            children: nested,
            label,
            label_timestamp,
        }
    }

    children
        .get(&None)
        .into_iter()
        .flatten()
        .map(|index| node(*index, entries, &children, &labels))
        .collect()
}

fn entry_base(entry: &SessionTreeEntry) -> &SessionTreeEntryBase {
    match entry {
        SessionTreeEntry::Message(value) => &value.base,
        SessionTreeEntry::ThinkingLevelChange(value) => &value.base,
        SessionTreeEntry::ModelChange(value) => &value.base,
        SessionTreeEntry::ActiveToolsChange(value) => &value.base,
        SessionTreeEntry::Compaction(value) => &value.base,
        SessionTreeEntry::BranchSummary(value) => &value.base,
        SessionTreeEntry::Custom(value) => &value.base,
        SessionTreeEntry::CustomMessage(value) => &value.base,
        SessionTreeEntry::Label(value) => &value.base,
        SessionTreeEntry::SessionInfo(value) => &value.base,
        SessionTreeEntry::Leaf(value) => &value.base,
    }
}

/// Pi lists sessions by the timestamp of their last user or assistant message,
/// falling back to the file modification time for header-only sessions.
#[must_use]
pub fn session_modified_timestamp(
    file_modified: SystemTime,
    message_timestamps: impl IntoIterator<Item = Option<SystemTime>>,
) -> SystemTime {
    message_timestamps
        .into_iter()
        .flatten()
        .last()
        .unwrap_or(file_modified)
}

/// Read selector metadata from a Pi JSONL session. Invalid files are rejected;
/// callers listing a directory can skip them like Pi does.
pub fn load_session_info(path: impl AsRef<Path>) -> io::Result<SessionInfo> {
    let path = path.as_ref();
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let mut info = SessionInfo::persisted("", path.to_string_lossy(), "");
    let mut message_timestamps = Vec::new();

    for (index, line) in BufReader::new(file).lines().enumerate() {
        let value: serde_json::Value = serde_json::from_str(&line?)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if index == 0 {
            if value.get("type").and_then(|v| v.as_str()) != Some("session") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing session header",
                ));
            }
            info.session_id = value
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into();
            info.cwd = value
                .get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .into();
            info.parent_session_path = value
                .get("parentSession")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            info.created = value
                .get("timestamp")
                .and_then(|value| value.as_str())
                .map(str::to_owned);
            continue;
        }
        match value.get("type").and_then(|v| v.as_str()) {
            Some("session_info") => {
                info.name = value
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned);
            }
            Some("message") => {
                info.message_count += 1;
                message_timestamps.push(None);
                if let Some(text) = message_text(&value) {
                    if info.first_message == "(no messages)" {
                        info.first_message.clone_from(&text);
                    }
                    if !info.all_messages_text.is_empty() {
                        info.all_messages_text.push(' ');
                    }
                    info.all_messages_text.push_str(&text);
                }
            }
            _ => {}
        }
    }
    if info.session_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing session id",
        ));
    }
    info.modified = Some(session_modified_timestamp(
        metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        message_timestamps,
    ));
    Ok(info)
}

pub fn list_session_infos(
    dir: impl AsRef<Path>,
    cwd: Option<&Path>,
    mut on_progress: impl FnMut(usize, usize),
) -> io::Result<Vec<SessionInfo>> {
    let mut files = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "jsonl")
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    files.sort();
    let total = files.len();
    let mut sessions = Vec::new();
    for (index, path) in files.into_iter().enumerate() {
        if let Ok(info) = load_session_info(path)
            && cwd.is_none_or(|cwd| Path::new(&info.cwd) == cwd)
        {
            sessions.push(info);
        }
        on_progress(index + 1, total);
    }
    sessions.sort_by(|left, right| right.modified.cmp(&left.modified));
    Ok(sessions)
}

pub fn delete_session(path: impl AsRef<Path>) -> io::Result<()> {
    fs::remove_file(path)
}

pub fn set_session_name(path: impl AsRef<Path>, name: &str) -> io::Result<()> {
    let path = path.as_ref();
    let name = name.replace(['\r', '\n'], " ").trim().to_owned();
    let parent_id = BufReader::new(fs::File::open(path)?)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
        .filter(|value| value.get("type").and_then(|kind| kind.as_str()) != Some("session"))
        .filter_map(|value| {
            value
                .get("id")
                .and_then(|id| id.as_str())
                .map(str::to_owned)
        })
        .last();
    let entry = serde_json::json!({
        "type": "session_info",
        "id": create_session_id(),
        "parentId": parent_id,
        "timestamp": create_timestamp(),
        "name": name,
    });
    writeln!(OpenOptions::new().append(true).open(path)?, "{entry}")
}

fn message_text(value: &serde_json::Value) -> Option<String> {
    let message = value.get("message")?;
    let content = message.get("content")?;
    match content {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
    .filter(|text| !text.is_empty())
}
