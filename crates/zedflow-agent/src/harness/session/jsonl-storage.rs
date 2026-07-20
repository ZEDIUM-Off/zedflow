//! JSONL session storage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::harness::types::{
    FileContent, FileSystem, JsonlSessionMetadata, LeafEntry, ReadTextLinesOptions, SessionError,
    SessionErrorCode, SessionStorage, SessionTreeEntry,
};

use super::repo_utils::{
    create_entry_base, create_timestamp, entry_id, entry_parent_id, entry_type_name,
    get_file_system_result_or_throw, leaf_id_after_entry,
};
use super::uuid::uuidv7;

/// Filesystem handle used by JSONL session storage.
pub type JsonlSessionStorageFileSystem = Arc<dyn FileSystem>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionHeader {
    #[serde(rename = "type")]
    header_type: String,
    version: u8,
    id: String,
    timestamp: String,
    cwd: String,
    #[serde(rename = "parentSession", skip_serializing_if = "Option::is_none")]
    parent_session: Option<String>,
}

#[derive(Debug, Clone)]
struct JsonlState {
    entries: Vec<SessionTreeEntry>,
    by_id: HashMap<String, SessionTreeEntry>,
    labels_by_id: HashMap<String, String>,
    current_leaf_id: Option<String>,
}

/// Session storage backed by a Pi-compatible JSONL file.
pub struct JsonlSessionStorage {
    fs: JsonlSessionStorageFileSystem,
    file_path: String,
    metadata: JsonlSessionMetadata,
    state: Mutex<JsonlState>,
}

impl JsonlSessionStorage {
    /// Open an existing JSONL session file.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the file cannot be read or parsed.
    pub async fn open(
        fs: JsonlSessionStorageFileSystem,
        file_path: impl Into<String>,
    ) -> Result<Self, SessionError> {
        let file_path = file_path.into();
        let loaded = load_jsonl_storage(fs.as_ref(), &file_path).await?;
        Ok(Self::from_loaded(
            fs,
            file_path,
            loaded.header,
            loaded.entries,
            loaded.leaf_id,
        ))
    }

    /// Create a JSONL session file and return storage for it.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the file cannot be written.
    pub async fn create(
        fs: JsonlSessionStorageFileSystem,
        file_path: impl Into<String>,
        options: JsonlSessionStorageCreateOptions,
    ) -> Result<Self, SessionError> {
        let file_path = file_path.into();
        let header = SessionHeader {
            header_type: "session".to_string(),
            version: 3,
            id: options.session_id,
            timestamp: create_timestamp(),
            cwd: options.cwd,
            parent_session: options.parent_session_path,
        };
        let line = serde_json::to_string(&header).map_err(|error| {
            SessionError::new(
                SessionErrorCode::Storage,
                "failed to serialize session header",
                Some(Box::new(error)),
            )
        })?;
        get_file_system_result_or_throw(
            fs.write_file(&file_path, FileContent::Text(format!("{line}\n")), None)
                .await,
            &format!("Failed to create session {file_path}"),
        )?;
        Ok(Self::from_loaded(fs, file_path, header, Vec::new(), None))
    }

    fn from_loaded(
        fs: JsonlSessionStorageFileSystem,
        file_path: String,
        header: SessionHeader,
        entries: Vec<SessionTreeEntry>,
        leaf_id: Option<String>,
    ) -> Self {
        let by_id = entries
            .iter()
            .map(|entry| (entry_id(entry).to_string(), entry.clone()))
            .collect::<HashMap<_, _>>();
        let labels_by_id = build_labels_by_id(&entries);
        let metadata = header_to_session_metadata(&header, &file_path);
        Self {
            fs,
            file_path,
            metadata,
            state: Mutex::new(JsonlState {
                entries,
                by_id,
                labels_by_id,
                current_leaf_id: leaf_id,
            }),
        }
    }
}

/// Options for creating JSONL storage directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlSessionStorageCreateOptions {
    /// Working directory stored in the session header.
    pub cwd: String,
    /// Session id stored in the session header.
    pub session_id: String,
    /// Optional parent session path.
    pub parent_session_path: Option<String>,
}

impl SessionStorage for JsonlSessionStorage {
    fn get_metadata<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, crate::harness::types::SessionMetadata> {
        Box::pin(async move { self.metadata.base.clone() })
    }

    fn get_leaf_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("jsonl session storage lock")
                .current_leaf_id
                .clone()
        })
    }

    fn set_leaf_id<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        Box::pin(async move {
            let entry = {
                let state = self.state.lock().expect("jsonl session storage lock");
                if let Some(id) = leaf_id.as_ref().filter(|id| !state.by_id.contains_key(*id)) {
                    return Err(SessionError::new(
                        SessionErrorCode::NotFound,
                        format!("Entry {id} not found"),
                        None,
                    ));
                }
                SessionTreeEntry::Leaf(LeafEntry {
                    base: create_entry_base(
                        generate_entry_id(&state.by_id),
                        state.current_leaf_id.clone(),
                    ),
                    target_id: leaf_id.clone(),
                })
            };
            append_jsonl_entry(self.fs.as_ref(), &self.file_path, &entry, "leaf").await?;
            let mut state = self.state.lock().expect("jsonl session storage lock");
            state
                .by_id
                .insert(entry_id(&entry).to_string(), entry.clone());
            state.entries.push(entry);
            state.current_leaf_id = leaf_id;
            Ok(())
        })
    }

    fn create_entry_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, String> {
        Box::pin(async move {
            let state = self.state.lock().expect("jsonl session storage lock");
            generate_entry_id(&state.by_id)
        })
    }

    fn append_entry<'a>(
        &'a self,
        entry: SessionTreeEntry,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        Box::pin(async move {
            append_jsonl_entry(self.fs.as_ref(), &self.file_path, &entry, "entry").await?;
            let mut state = self.state.lock().expect("jsonl session storage lock");
            update_label_cache(&mut state.labels_by_id, &entry);
            state.current_leaf_id = leaf_id_after_entry(&entry);
            state
                .by_id
                .insert(entry_id(&entry).to_string(), entry.clone());
            state.entries.push(entry);
            Ok(())
        })
    }

    fn get_entry<'a>(
        &'a self,
        id: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Option<SessionTreeEntry>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("jsonl session storage lock")
                .by_id
                .get(id)
                .cloned()
        })
    }

    fn find_entries<'a>(
        &'a self,
        entry_type: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("jsonl session storage lock")
                .entries
                .iter()
                .filter(|entry| entry_type_name(entry) == entry_type)
                .cloned()
                .collect()
        })
    }

    fn get_label<'a>(
        &'a self,
        id: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("jsonl session storage lock")
                .labels_by_id
                .get(id)
                .cloned()
        })
    }

    fn get_path_to_root<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move {
            let Some(leaf_id) = leaf_id else {
                return Vec::new();
            };
            let state = self.state.lock().expect("jsonl session storage lock");
            path_to_root(&state.by_id, &leaf_id).unwrap_or_default()
        })
    }

    fn get_entries<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("jsonl session storage lock")
                .entries
                .clone()
        })
    }
}

/// Load only JSONL session metadata from a file.
///
/// # Errors
///
/// Returns [`SessionError`] when the header is missing, unreadable, or invalid.
pub async fn load_jsonl_session_metadata(
    fs: &dyn FileSystem,
    file_path: &str,
) -> Result<JsonlSessionMetadata, SessionError> {
    let lines = get_file_system_result_or_throw(
        fs.read_text_lines(
            file_path,
            ReadTextLinesOptions {
                max_lines: Some(1),
                abort_signal: None,
            },
        )
        .await,
        &format!("Failed to read session header {file_path}"),
    )?;
    match lines.first().filter(|line| !line.trim().is_empty()) {
        Some(line) => Ok(header_to_session_metadata(
            &parse_header_line(line, file_path)?,
            file_path,
        )),
        None => Err(invalid_session(file_path, "missing session header", None)),
    }
}

struct LoadedJsonlStorage {
    header: SessionHeader,
    entries: Vec<SessionTreeEntry>,
    leaf_id: Option<String>,
}

async fn load_jsonl_storage(
    fs: &dyn FileSystem,
    file_path: &str,
) -> Result<LoadedJsonlStorage, SessionError> {
    let content = get_file_system_result_or_throw(
        fs.read_text_file(file_path, None).await,
        &format!("Failed to read session {file_path}"),
    )?;
    let lines = content
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let Some(header_line) = lines.first() else {
        return Err(invalid_session(file_path, "missing session header", None));
    };
    let header = parse_header_line(header_line, file_path)?;
    let mut entries = Vec::new();
    let mut leaf_id = None;
    for (index, line) in lines.iter().enumerate().skip(1) {
        let entry = parse_entry_line(line, file_path, index + 1)?;
        leaf_id = leaf_id_after_entry(&entry);
        entries.push(entry);
    }
    Ok(LoadedJsonlStorage {
        header,
        entries,
        leaf_id,
    })
}

fn parse_header_line(line: &str, file_path: &str) -> Result<SessionHeader, SessionError> {
    let parsed = serde_json::from_str::<serde_json::Value>(line).map_err(|error| {
        invalid_session(
            file_path,
            "first line is not a valid session header",
            Some(Box::new(error)),
        )
    })?;
    if !parsed.is_object() {
        return Err(invalid_session(
            file_path,
            "first line is not a valid session header",
            None,
        ));
    }
    let header = serde_json::from_value::<SessionHeader>(parsed).map_err(|error| {
        invalid_session(
            file_path,
            "first line is not a valid session header",
            Some(Box::new(error)),
        )
    })?;
    if header.header_type != "session" {
        return Err(invalid_session(
            file_path,
            "first line is not a valid session header",
            None,
        ));
    }
    if header.version != 3 {
        return Err(invalid_session(
            file_path,
            "unsupported session version",
            None,
        ));
    }
    if header.id.is_empty() {
        return Err(invalid_session(
            file_path,
            "session header is missing id",
            None,
        ));
    }
    if header.timestamp.is_empty() {
        return Err(invalid_session(
            file_path,
            "session header is missing timestamp",
            None,
        ));
    }
    if header.cwd.is_empty() {
        return Err(invalid_session(
            file_path,
            "session header is missing cwd",
            None,
        ));
    }
    Ok(header)
}

fn parse_entry_line(
    line: &str,
    file_path: &str,
    line_number: usize,
) -> Result<SessionTreeEntry, SessionError> {
    let parsed = serde_json::from_str::<serde_json::Value>(line).map_err(|error| {
        invalid_entry(
            file_path,
            line_number,
            "is not valid JSON",
            Some(Box::new(error)),
        )
    })?;
    let Some(object) = parsed.as_object() else {
        return Err(invalid_entry(
            file_path,
            line_number,
            "is not a valid session entry",
            None,
        ));
    };
    if !object.get("type").is_some_and(serde_json::Value::is_string) {
        return Err(invalid_entry(
            file_path,
            line_number,
            "is missing entry type",
            None,
        ));
    }
    if !object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|id| !id.is_empty())
    {
        return Err(invalid_entry(
            file_path,
            line_number,
            "is missing entry id",
            None,
        ));
    }
    if object
        .get("parentId")
        .is_some_and(|parent_id| !parent_id.is_null() && !parent_id.is_string())
    {
        return Err(invalid_entry(
            file_path,
            line_number,
            "has invalid parentId",
            None,
        ));
    }
    if !object
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|timestamp| !timestamp.is_empty())
    {
        return Err(invalid_entry(
            file_path,
            line_number,
            "is missing timestamp",
            None,
        ));
    }
    if object.get("type").and_then(serde_json::Value::as_str) == Some("leaf")
        && object
            .get("targetId")
            .is_some_and(|target_id| !target_id.is_null() && !target_id.is_string())
    {
        return Err(invalid_entry(
            file_path,
            line_number,
            "has invalid targetId",
            None,
        ));
    }
    serde_json::from_value(parsed).map_err(|error| {
        invalid_entry(
            file_path,
            line_number,
            "is not a valid session entry",
            Some(Box::new(error)),
        )
    })
}

fn header_to_session_metadata(header: &SessionHeader, path: &str) -> JsonlSessionMetadata {
    JsonlSessionMetadata {
        base: crate::harness::types::SessionMetadata {
            id: header.id.clone(),
            created_at: header.timestamp.clone(),
        },
        cwd: header.cwd.clone(),
        path: path.to_string(),
        parent_session_path: header.parent_session.clone(),
    }
}

async fn append_jsonl_entry(
    fs: &dyn FileSystem,
    file_path: &str,
    entry: &SessionTreeEntry,
    context: &str,
) -> Result<(), SessionError> {
    let line = serde_json::to_string(entry).map_err(|error| {
        SessionError::new(
            SessionErrorCode::Storage,
            format!("Failed to serialize session entry {}", entry_id(entry)),
            Some(Box::new(error)),
        )
    })?;
    get_file_system_result_or_throw(
        fs.append_file(file_path, FileContent::Text(format!("{line}\n")), None)
            .await,
        &format!("Failed to append session {context} {}", entry_id(entry)),
    )
}

fn invalid_session(
    file_path: &str,
    message: &str,
    source: Option<crate::harness::types::SessionErrorSource>,
) -> SessionError {
    SessionError::new(
        SessionErrorCode::InvalidSession,
        format!("Invalid JSONL session file {file_path}: {message}"),
        source,
    )
}

fn invalid_entry(
    file_path: &str,
    line_number: usize,
    message: &str,
    source: Option<crate::harness::types::SessionErrorSource>,
) -> SessionError {
    SessionError::new(
        SessionErrorCode::InvalidEntry,
        format!("Invalid JSONL session file {file_path}: line {line_number} {message}"),
        source,
    )
}

fn update_label_cache(labels_by_id: &mut HashMap<String, String>, entry: &SessionTreeEntry) {
    let SessionTreeEntry::Label(entry) = entry else {
        return;
    };
    match entry
        .label
        .as_deref()
        .map(str::trim)
        .filter(|label| !label.is_empty())
    {
        Some(label) => {
            labels_by_id.insert(entry.target_id.clone(), label.to_string());
        }
        None => {
            labels_by_id.remove(&entry.target_id);
        }
    }
}

fn build_labels_by_id(entries: &[SessionTreeEntry]) -> HashMap<String, String> {
    let mut labels_by_id = HashMap::new();
    for entry in entries {
        update_label_cache(&mut labels_by_id, entry);
    }
    labels_by_id
}

fn generate_entry_id(by_id: &HashMap<String, SessionTreeEntry>) -> String {
    for _ in 0..100 {
        let id = uuidv7().chars().rev().take(8).collect::<String>();
        let id = id.chars().rev().collect::<String>();
        if !by_id.contains_key(&id) {
            return id;
        }
    }
    uuidv7()
}

fn path_to_root(
    by_id: &HashMap<String, SessionTreeEntry>,
    leaf_id: &str,
) -> Result<Vec<SessionTreeEntry>, SessionError> {
    let mut path = Vec::new();
    let mut current = by_id.get(leaf_id).cloned().ok_or_else(|| {
        SessionError::new(
            SessionErrorCode::NotFound,
            format!("Entry {leaf_id} not found"),
            None,
        )
    })?;
    loop {
        path.push(current.clone());
        let Some(parent_id) = entry_parent_id(&current) else {
            break;
        };
        current = by_id.get(parent_id).cloned().ok_or_else(|| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("Entry {parent_id} not found"),
                None,
            )
        })?;
    }
    path.reverse();
    Ok(path)
}
