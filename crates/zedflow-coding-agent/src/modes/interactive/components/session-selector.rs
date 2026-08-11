//! Session picker state and actions, ported from Pi's interactive selector.

use std::{io, path::PathBuf};

use crate::{
    session_manager::{self, SessionInfo},
    session_selector_search::{
        NameFilter, SessionInfo as SearchSessionInfo, SortMode, filter_and_sort_sessions,
    },
    utils::paths::canonicalize_path,
};

/// Keys that can request deletion from the session picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSelectorKey {
    CtrlD,
    CtrlBackspace,
    Other,
}

/// Whether a key should open deletion confirmation.
#[must_use]
pub fn should_confirm_delete(key: SessionSelectorKey, search_query: &str) -> bool {
    key == SessionSelectorKey::CtrlD
        || (key == SessionSelectorKey::CtrlBackspace && search_query.is_empty())
}

/// Pi's rename editor starts at the beginning of the existing session name.
#[must_use]
pub fn renamed_session_name(typed: &str, existing: &str) -> String {
    format!("{typed}{existing}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSelectorAction {
    None,
    Select(PathBuf),
    Cancel,
    ConfirmDelete(PathBuf),
    Deleted(PathBuf),
    Rename(PathBuf),
    Error(String),
}

/// Deterministic state behind both `/resume` and the startup session picker.
#[derive(Debug, Clone)]
pub struct SessionSelectorState {
    sessions: Vec<SessionInfo>,
    visible: Vec<usize>,
    selected: usize,
    query: String,
    sort_mode: SortMode,
    name_filter: NameFilter,
    current_session: Option<PathBuf>,
    confirming_delete: Option<PathBuf>,
}

impl SessionSelectorState {
    #[must_use]
    pub fn new(sessions: Vec<SessionInfo>, current_session: Option<PathBuf>) -> Self {
        let mut state = Self {
            sessions,
            visible: Vec::new(),
            selected: 0,
            query: String::new(),
            sort_mode: SortMode::Threaded,
            name_filter: NameFilter::All,
            current_session: current_session.map(canonicalize_path),
            confirming_delete: None,
        };
        state.refresh();
        state
    }

    #[must_use]
    pub fn visible_sessions(&self) -> impl Iterator<Item = &SessionInfo> {
        self.visible.iter().map(|index| &self.sessions[*index])
    }

    #[must_use]
    pub fn selected_session(&self) -> Option<&SessionInfo> {
        self.visible
            .get(self.selected)
            .map(|index| &self.sessions[*index])
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.refresh();
    }

    pub fn set_sort_mode(&mut self, mode: SortMode) {
        self.sort_mode = mode;
        self.refresh();
    }

    pub fn set_name_filter(&mut self, filter: NameFilter) {
        self.name_filter = filter;
        self.refresh();
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.visible.len().saturating_sub(1));
    }

    #[must_use]
    pub fn select(&self) -> SessionSelectorAction {
        self.selected_path()
            .map_or(SessionSelectorAction::None, SessionSelectorAction::Select)
    }

    #[must_use]
    pub const fn cancel(&self) -> SessionSelectorAction {
        SessionSelectorAction::Cancel
    }

    #[must_use]
    pub fn request_rename(&self) -> SessionSelectorAction {
        self.selected_path()
            .map_or(SessionSelectorAction::None, SessionSelectorAction::Rename)
    }

    pub fn rename_selected(&mut self, name: &str) -> io::Result<()> {
        let Some(index) = self.visible.get(self.selected).copied() else {
            return Ok(());
        };
        let Some(path) = self.sessions[index].session_file.clone() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "session is not persisted",
            ));
        };
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }
        session_manager::set_session_name(&path, name)?;
        self.sessions[index].name = Some(name.to_owned());
        self.refresh();
        Ok(())
    }

    #[must_use]
    pub fn request_delete(&mut self) -> SessionSelectorAction {
        let Some(path) = self.selected_path() else {
            return SessionSelectorAction::None;
        };
        if self
            .current_session
            .as_ref()
            .is_some_and(|current| current == &canonicalize_path(&path))
        {
            return SessionSelectorAction::Error(
                "Cannot delete the currently active session".into(),
            );
        }
        self.confirming_delete = Some(path.clone());
        SessionSelectorAction::ConfirmDelete(path)
    }

    pub fn cancel_delete(&mut self) {
        self.confirming_delete = None;
    }

    pub fn confirm_delete(&mut self) -> io::Result<SessionSelectorAction> {
        let Some(path) = self.confirming_delete.take() else {
            return Ok(SessionSelectorAction::None);
        };
        session_manager::delete_session(&path)?;
        self.sessions.retain(|session| {
            session.session_file.as_deref() != Some(path.to_string_lossy().as_ref())
        });
        self.refresh();
        Ok(SessionSelectorAction::Deleted(path))
    }

    fn selected_path(&self) -> Option<PathBuf> {
        self.selected_session()
            .and_then(|session| session.session_file.as_ref())
            .map(PathBuf::from)
    }

    fn refresh(&mut self) {
        let searchable = self
            .sessions
            .iter()
            .map(|session| SearchSessionInfo {
                id: session.session_id.clone(),
                name: session.name.clone(),
                all_messages_text: session.all_messages_text.clone(),
                cwd: session.cwd.clone(),
                modified: session
                    .modified
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            })
            .collect::<Vec<_>>();
        self.visible =
            filter_and_sort_sessions(&searchable, &self.query, self.sort_mode, self.name_filter)
                .into_iter()
                .filter_map(|matched| {
                    self.sessions
                        .iter()
                        .position(|session| session.session_id == matched.id)
                })
                .collect();
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }
}
