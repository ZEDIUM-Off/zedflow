//! In-memory session storage.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::harness::types::{
    LeafEntry, SessionError, SessionErrorCode, SessionMetadata, SessionStorage, SessionTreeEntry,
};

use super::repo_utils::{
    create_entry_base, create_session_id, create_timestamp, entry_id, entry_parent_id,
    entry_type_name, leaf_id_after_entry,
};
use super::uuid::uuidv7;

/// Constructor options for [`InMemorySessionStorage`].
#[derive(Debug, Clone, Default)]
pub struct InMemorySessionStorageOptions {
    /// Initial entries.
    pub entries: Option<Vec<SessionTreeEntry>>,
    /// Metadata to use instead of creating fresh metadata.
    pub metadata: Option<SessionMetadata>,
}

#[derive(Debug, Clone)]
struct MemoryState {
    entries: Vec<SessionTreeEntry>,
    by_id: HashMap<String, SessionTreeEntry>,
    labels_by_id: HashMap<String, String>,
    leaf_id: Option<String>,
}

/// Session storage backed by process memory.
#[derive(Debug)]
pub struct InMemorySessionStorage {
    metadata: SessionMetadata,
    state: Mutex<MemoryState>,
}

impl InMemorySessionStorage {
    /// Create memory storage.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the initial leaf points at a missing entry.
    pub fn new(options: Option<InMemorySessionStorageOptions>) -> Result<Self, SessionError> {
        let options = options.unwrap_or_default();
        let entries = options.entries.unwrap_or_default();
        let by_id = entries
            .iter()
            .map(|entry| (entry_id(entry).to_string(), entry.clone()))
            .collect::<HashMap<_, _>>();
        let labels_by_id = build_labels_by_id(&entries);
        let leaf_id = entries.iter().filter_map(leaf_id_after_entry).last();
        if leaf_id.as_ref().is_some_and(|id| !by_id.contains_key(id)) {
            return Err(SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("Entry {} not found", leaf_id.unwrap_or_default()),
                None,
            ));
        }
        let metadata = options.metadata.unwrap_or_else(|| SessionMetadata {
            id: create_session_id(),
            created_at: create_timestamp(),
        });
        Ok(Self {
            metadata,
            state: Mutex::new(MemoryState {
                entries,
                by_id,
                labels_by_id,
                leaf_id,
            }),
        })
    }
}

impl Default for InMemorySessionStorage {
    fn default() -> Self {
        Self::new(None).expect("empty in-memory session storage is valid")
    }
}

impl SessionStorage for InMemorySessionStorage {
    fn get_metadata<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, SessionMetadata> {
        Box::pin(async move { self.metadata.clone() })
    }

    fn get_leaf_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("session storage lock")
                .leaf_id
                .clone()
        })
    }

    fn set_leaf_id<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, ()> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("session storage lock");
            if leaf_id
                .as_ref()
                .is_some_and(|id| !state.by_id.contains_key(id))
            {
                return;
            }
            let entry = SessionTreeEntry::Leaf(LeafEntry {
                base: create_entry_base(generate_entry_id(&state.by_id), state.leaf_id.clone()),
                target_id: leaf_id.clone(),
            });
            state
                .by_id
                .insert(entry_id(&entry).to_string(), entry.clone());
            state.entries.push(entry);
            state.leaf_id = leaf_id;
        })
    }

    fn create_entry_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, String> {
        Box::pin(async move {
            let state = self.state.lock().expect("session storage lock");
            generate_entry_id(&state.by_id)
        })
    }

    fn append_entry<'a>(
        &'a self,
        entry: SessionTreeEntry,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        Box::pin(async move {
            let mut state = self.state.lock().expect("session storage lock");
            update_label_cache(&mut state.labels_by_id, &entry);
            state.leaf_id = leaf_id_after_entry(&entry);
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
                .expect("session storage lock")
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
                .expect("session storage lock")
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
                .expect("session storage lock")
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
            let state = self.state.lock().expect("session storage lock");
            path_to_root(&state.by_id, &leaf_id).unwrap_or_default()
        })
    }

    fn get_entries<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move {
            self.state
                .lock()
                .expect("session storage lock")
                .entries
                .clone()
        })
    }
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
