//! Pi-compatible session-selector interactions.

/// Keys that can request deletion from the session picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSelectorKey {
    CtrlD,
    CtrlBackspace,
    Other,
}

/// Whether a key should open deletion confirmation.
///
/// Ctrl+Backspace edits a non-empty search query; Ctrl+D always deletes the
/// selected session, matching Pi's picker interaction.
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
