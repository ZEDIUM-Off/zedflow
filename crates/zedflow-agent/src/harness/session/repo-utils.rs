//! Shared session repository helpers.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::harness::session::session::Session as SessionImpl;
use crate::harness::types::{
    FileError, SessionError, SessionErrorCode, SessionForkOptions, SessionForkPosition,
    SessionMetadata, SessionStorage, SessionTreeEntry, SessionTreeEntryBase,
};
use crate::types::{AgentMessage, Message};
use zedflow_ai::UserMessageContent;

use super::uuid::uuidv7;

/// Create a new session id.
#[must_use]
pub fn create_session_id() -> String {
    uuidv7()
}

/// Create an ISO-8601 UTC timestamp.
#[must_use]
pub fn create_timestamp() -> String {
    unix_millis_to_iso(now_millis())
}

/// Return the current Unix time in milliseconds.
#[must_use]
pub fn now_millis() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_secs().saturating_mul(1_000) + u64::from(duration.subsec_millis())
}

/// Convert Unix milliseconds to the timestamp shape written by Pi sessions.
#[must_use]
pub fn unix_millis_to_iso(millis: u64) -> String {
    let secs = millis / 1_000;
    let ms = millis % 1_000;
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = sod / 3_600;
    let minute = (sod % 3_600) / 60;
    let second = sod % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z")
}

/// Parse the UTC timestamps written by this module into Unix milliseconds.
#[must_use]
pub fn iso_to_unix_millis(value: &str) -> Option<u64> {
    let year: i32 = value.get(0..4)?.parse().ok()?;
    let month: u32 = value.get(5..7)?.parse().ok()?;
    let day: u32 = value.get(8..10)?.parse().ok()?;
    let hour: u64 = value.get(11..13)?.parse().ok()?;
    let minute: u64 = value.get(14..16)?.parse().ok()?;
    let second: u64 = value.get(17..19)?.parse().ok()?;
    let millis = if value.get(19..20) == Some(".") {
        value.get(20..23)?.parse().ok()?
    } else {
        0
    };
    let days = days_from_civil(year, month, day)?;
    let secs = u64::try_from(days).ok()?.saturating_mul(86_400)
        + hour.saturating_mul(3_600)
        + minute.saturating_mul(60)
        + second;
    Some(secs.saturating_mul(1_000) + millis)
}

/// Wrap a storage object in the concrete session implementation.
#[must_use]
pub fn to_session<TStorage>(storage: TStorage) -> SessionImpl<TStorage>
where
    TStorage: SessionStorage,
{
    SessionImpl::new(storage)
}

/// Wrap shared storage in the concrete session implementation.
#[must_use]
pub fn to_shared_session<TStorage>(storage: Arc<TStorage>) -> SessionImpl<Arc<TStorage>>
where
    TStorage: SessionStorage,
{
    SessionImpl::new(storage)
}

/// Convert filesystem failures into session storage failures.
///
/// # Errors
///
/// Returns [`SessionError`] with `not_found` for missing paths and `storage` for other file errors.
pub fn get_file_system_result_or_throw<TValue>(
    result: Result<TValue, FileError>,
    message: &str,
) -> Result<TValue, SessionError> {
    result.map_err(|error| {
        let code = if error.code == crate::harness::types::FileErrorCode::NotFound {
            SessionErrorCode::NotFound
        } else {
            SessionErrorCode::Storage
        };
        SessionError::new(
            code,
            format!("{message}: {}", error.message),
            Some(Box::new(error)),
        )
    })
}

/// Return the source branch entries that should be copied into a fork.
///
/// # Errors
///
/// Returns [`SessionError`] when the requested fork target is missing or invalid.
pub async fn get_entries_to_fork<TStorage>(
    storage: &TStorage,
    options: &SessionForkOptions,
) -> Result<Vec<SessionTreeEntry>, SessionError>
where
    TStorage: SessionStorage + ?Sized,
{
    let Some(entry_id) = options.entry_id.as_deref() else {
        return Ok(storage.get_entries().await);
    };
    let target = storage.get_entry(entry_id).await.ok_or_else(|| {
        SessionError::new(
            SessionErrorCode::InvalidForkTarget,
            format!("Entry {entry_id} not found"),
            None,
        )
    })?;
    let effective_leaf_id =
        if options.position.unwrap_or(SessionForkPosition::Before) == SessionForkPosition::At {
            Some(entry_id.to_string())
        } else {
            if !entry_is_user_message(&target) {
                return Err(SessionError::new(
                    SessionErrorCode::InvalidForkTarget,
                    format!("Entry {entry_id} is not a user message"),
                    None,
                ));
            }
            entry_parent_id(&target).cloned()
        };
    Ok(storage.get_path_to_root(effective_leaf_id).await)
}

/// Return the entry type name used in Pi JSONL.
#[must_use]
pub fn entry_type_name(entry: &SessionTreeEntry) -> &'static str {
    match entry {
        SessionTreeEntry::Message(_) => "message",
        SessionTreeEntry::ThinkingLevelChange(_) => "thinking_level_change",
        SessionTreeEntry::ModelChange(_) => "model_change",
        SessionTreeEntry::ActiveToolsChange(_) => "active_tools_change",
        SessionTreeEntry::Compaction(_) => "compaction",
        SessionTreeEntry::BranchSummary(_) => "branch_summary",
        SessionTreeEntry::Custom(_) => "custom",
        SessionTreeEntry::CustomMessage(_) => "custom_message",
        SessionTreeEntry::Label(_) => "label",
        SessionTreeEntry::SessionInfo(_) => "session_info",
        SessionTreeEntry::Leaf(_) => "leaf",
    }
}

/// Return an entry id.
#[must_use]
pub fn entry_id(entry: &SessionTreeEntry) -> &str {
    &entry_base(entry).id
}

/// Return an entry parent id.
#[must_use]
pub fn entry_parent_id(entry: &SessionTreeEntry) -> Option<&String> {
    entry_base(entry).parent_id.as_ref()
}

/// Return the leaf id after applying an entry.
#[must_use]
pub fn leaf_id_after_entry(entry: &SessionTreeEntry) -> Option<String> {
    match entry {
        SessionTreeEntry::Leaf(entry) => entry.target_id.clone(),
        _ => Some(entry_id(entry).to_string()),
    }
}

/// Create common entry fields for a new child of `parent_id`.
#[must_use]
pub fn create_entry_base(id: String, parent_id: Option<String>) -> SessionTreeEntryBase {
    SessionTreeEntryBase {
        id,
        parent_id,
        timestamp: create_timestamp(),
    }
}

/// Return true when `entry` is a user message entry.
#[must_use]
pub fn entry_is_user_message(entry: &SessionTreeEntry) -> bool {
    let SessionTreeEntry::Message(entry) = entry else {
        return false;
    };
    match &entry.message {
        AgentMessage::Llm(Message::User(_)) => true,
        AgentMessage::Llm(_) => false,
        AgentMessage::Custom(value) => value
            .get("role")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|role| role == "user"),
    }
}

/// Return true when an agent message is assistant-authored.
#[must_use]
pub fn message_is_assistant(message: &AgentMessage) -> bool {
    match message {
        AgentMessage::Llm(Message::Assistant(_)) => true,
        AgentMessage::Llm(_) => false,
        AgentMessage::Custom(value) => value
            .get("role")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|role| role == "assistant"),
    }
}

/// Return text from a user message when available.
#[must_use]
pub fn user_message_text(message: &AgentMessage) -> Option<&str> {
    match message {
        AgentMessage::Llm(Message::User(user)) => match &user.content {
            UserMessageContent::Text(text) => Some(text),
            UserMessageContent::Blocks(_) => None,
        },
        AgentMessage::Llm(_) => None,
        AgentMessage::Custom(value) => value.get("content")?.as_str(),
    }
}

fn entry_base(entry: &SessionTreeEntry) -> &SessionTreeEntryBase {
    match entry {
        SessionTreeEntry::Message(entry) => &entry.base,
        SessionTreeEntry::ThinkingLevelChange(entry) => &entry.base,
        SessionTreeEntry::ModelChange(entry) => &entry.base,
        SessionTreeEntry::ActiveToolsChange(entry) => &entry.base,
        SessionTreeEntry::Compaction(entry) => &entry.base,
        SessionTreeEntry::BranchSummary(entry) => &entry.base,
        SessionTreeEntry::Custom(entry) => &entry.base,
        SessionTreeEntry::CustomMessage(entry) => &entry.base,
        SessionTreeEntry::Label(entry) => &entry.base,
        SessionTreeEntry::SessionInfo(entry) => &entry.base,
        SessionTreeEntry::Leaf(entry) => &entry.base,
    }
}

fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month as u64, day as u64)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let y = i64::from(year) - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from(month) + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

impl<TStorage> SessionStorage for Arc<TStorage>
where
    TStorage: SessionStorage,
{
    fn get_metadata<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, SessionMetadata> {
        (**self).get_metadata()
    }

    fn get_leaf_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        (**self).get_leaf_id()
    }

    fn set_leaf_id<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        (**self).set_leaf_id(leaf_id)
    }

    fn create_entry_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, String> {
        (**self).create_entry_id()
    }

    fn append_entry<'a>(
        &'a self,
        entry: SessionTreeEntry,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        (**self).append_entry(entry)
    }

    fn get_entry<'a>(
        &'a self,
        id: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Option<SessionTreeEntry>> {
        (**self).get_entry(id)
    }

    fn find_entries<'a>(
        &'a self,
        entry_type: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        (**self).find_entries(entry_type)
    }

    fn get_label<'a>(
        &'a self,
        id: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        (**self).get_label(id)
    }

    fn get_path_to_root<'a>(
        &'a self,
        leaf_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        (**self).get_path_to_root(leaf_id)
    }

    fn get_entries<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        (**self).get_entries()
    }
}
