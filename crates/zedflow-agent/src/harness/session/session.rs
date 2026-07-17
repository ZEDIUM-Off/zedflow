//! Session tree behavior and context reconstruction.

use serde_json::{Value, json};

use crate::harness::types::{
    ActiveToolsChangeEntry, BranchSummaryDraft, BranchSummaryEntry, CompactionEntry, CustomEntry,
    CustomMessageContent, CustomMessageEntry, LabelEntry, MessageEntry, ModelChangeEntry,
    SessionContext, SessionError, SessionErrorCode, SessionMetadata, SessionModelRef,
    SessionStorage, SessionTreeEntry, SessionTreeEntryBase, ThinkingLevelChangeEntry,
};
use crate::types::{AgentMessage, Message};

use super::repo_utils::{
    create_entry_base, entry_id, iso_to_unix_millis, leaf_id_after_entry, message_is_assistant,
};

/// Reconstruct the model-visible context from a path of session entries.
#[must_use]
pub fn build_session_context(path_entries: &[SessionTreeEntry]) -> SessionContext {
    let mut thinking_level = "off".to_string();
    let mut model = None;
    let mut active_tool_names = None;
    let mut compaction = None;

    for entry in path_entries {
        match entry {
            SessionTreeEntry::ThinkingLevelChange(entry) => {
                thinking_level.clone_from(&entry.thinking_level);
            }
            SessionTreeEntry::ModelChange(entry) => {
                model = Some(SessionModelRef {
                    provider: entry.provider.clone(),
                    model_id: entry.model_id.clone(),
                });
            }
            SessionTreeEntry::Message(entry) if message_is_assistant(&entry.message) => {
                if let Some(model_ref) = assistant_model_ref(&entry.message) {
                    model = Some(model_ref);
                }
            }
            SessionTreeEntry::ActiveToolsChange(entry) => {
                active_tool_names = Some(entry.active_tool_names.clone());
            }
            SessionTreeEntry::Compaction(entry) => {
                compaction = Some(entry);
            }
            _ => {}
        }
    }

    let mut messages = Vec::new();
    if let Some(compaction) = compaction {
        messages.push(create_compaction_summary_message(
            &compaction.summary,
            compaction.tokens_before,
            &compaction.base.timestamp,
        ));
        let compaction_idx = path_entries.iter().position(|entry| {
            matches!(entry, SessionTreeEntry::Compaction(entry) if entry.base.id == compaction.base.id)
        });
        if let Some(compaction_idx) = compaction_idx {
            let mut found_first_kept = false;
            for entry in &path_entries[..compaction_idx] {
                if entry_id(entry) == compaction.first_kept_entry_id {
                    found_first_kept = true;
                }
                if found_first_kept {
                    append_context_message(&mut messages, entry);
                }
            }
            for entry in &path_entries[compaction_idx + 1..] {
                append_context_message(&mut messages, entry);
            }
        }
    } else {
        for entry in path_entries {
            append_context_message(&mut messages, entry);
        }
    }

    SessionContext {
        messages,
        thinking_level,
        model,
        active_tool_names,
    }
}

/// Concrete session facade over a [`SessionStorage`].
#[derive(Debug, Clone)]
pub struct Session<TStorage> {
    storage: TStorage,
}

impl<TStorage> Session<TStorage>
where
    TStorage: SessionStorage,
{
    /// Create a session over the provided storage backend.
    #[must_use]
    pub fn new(storage: TStorage) -> Self {
        Self { storage }
    }

    /// Return the backing storage.
    #[must_use]
    pub fn storage(&self) -> &TStorage {
        &self.storage
    }

    /// Return session metadata.
    pub async fn get_metadata(&self) -> SessionMetadata {
        self.storage.get_metadata().await
    }

    /// Return current leaf id.
    pub async fn get_leaf_id(&self) -> Option<String> {
        self.storage.get_leaf_id().await
    }

    /// Return an entry by id.
    pub async fn get_entry(&self, id: &str) -> Option<SessionTreeEntry> {
        self.storage.get_entry(id).await
    }

    /// Return all entries.
    pub async fn get_entries(&self) -> Vec<SessionTreeEntry> {
        self.storage.get_entries().await
    }

    /// Return branch entries from `from_id`, or from the active leaf.
    pub async fn get_branch(&self, from_id: Option<String>) -> Vec<SessionTreeEntry> {
        let leaf_id = match from_id {
            Some(id) => Some(id),
            None => self.storage.get_leaf_id().await,
        };
        self.storage.get_path_to_root(leaf_id).await
    }

    /// Build the current session context.
    pub async fn build_context(&self) -> SessionContext {
        build_session_context(&self.get_branch(None).await)
    }

    /// Return a label for `id`.
    pub async fn get_label(&self, id: &str) -> Option<String> {
        self.storage.get_label(id).await
    }

    /// Return the latest non-empty session name.
    pub async fn get_session_name(&self) -> Option<String> {
        self.storage
            .find_entries("session_info")
            .await
            .into_iter()
            .filter_map(|entry| match entry {
                SessionTreeEntry::SessionInfo(entry) => entry.name,
                _ => None,
            })
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .last()
    }

    /// Append a typed entry and return its id.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_typed_entry(
        &self,
        entry: SessionTreeEntry,
    ) -> Result<String, SessionError> {
        let id = entry_id(&entry).to_string();
        self.storage.append_entry(entry).await?;
        Ok(id)
    }

    /// Append an agent message.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_message(&self, message: AgentMessage) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::Message(MessageEntry {
            base: self.next_base().await,
            message,
        });
        self.append_typed_entry(entry).await
    }

    /// Append a thinking level change.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_thinking_level_change(
        &self,
        thinking_level: impl Into<String>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::ThinkingLevelChange(ThinkingLevelChangeEntry {
            base: self.next_base().await,
            thinking_level: thinking_level.into(),
        });
        self.append_typed_entry(entry).await
    }

    /// Append a model change.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_model_change(
        &self,
        provider: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::ModelChange(ModelChangeEntry {
            base: self.next_base().await,
            provider: provider.into(),
            model_id: model_id.into(),
        });
        self.append_typed_entry(entry).await
    }

    /// Append an active-tools change.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_active_tools_change(
        &self,
        active_tool_names: Vec<String>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::ActiveToolsChange(ActiveToolsChangeEntry {
            base: self.next_base().await,
            active_tool_names,
        });
        self.append_typed_entry(entry).await
    }

    /// Append a compaction marker.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_compaction(
        &self,
        summary: impl Into<String>,
        first_kept_entry_id: impl Into<String>,
        tokens_before: u64,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::Compaction(CompactionEntry {
            base: self.next_base().await,
            summary: summary.into(),
            first_kept_entry_id: first_kept_entry_id.into(),
            tokens_before,
            details,
            from_hook,
        });
        self.append_typed_entry(entry).await
    }

    /// Append a custom data entry.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_custom_entry(
        &self,
        custom_type: impl Into<String>,
        data: Option<Value>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::Custom(CustomEntry {
            base: self.next_base().await,
            custom_type: custom_type.into(),
            data,
        });
        self.append_typed_entry(entry).await
    }

    /// Append a custom message entry.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_custom_message_entry(
        &self,
        custom_type: impl Into<String>,
        content: CustomMessageContent,
        display: bool,
        details: Option<Value>,
    ) -> Result<String, SessionError> {
        let entry = SessionTreeEntry::CustomMessage(CustomMessageEntry {
            base: self.next_base().await,
            custom_type: custom_type.into(),
            content,
            details,
            display,
        });
        self.append_typed_entry(entry).await
    }

    /// Append or clear a label for an entry.
    ///
    /// # Errors
    ///
    /// Returns `not_found` when `target_id` does not exist, or storage errors from the backing storage.
    pub async fn append_label(
        &self,
        target_id: impl Into<String>,
        label: Option<String>,
    ) -> Result<String, SessionError> {
        let target_id = target_id.into();
        if self.storage.get_entry(&target_id).await.is_none() {
            return Err(SessionError::new(
                SessionErrorCode::NotFound,
                format!("Entry {target_id} not found"),
                None,
            ));
        }
        let entry = SessionTreeEntry::Label(LabelEntry {
            base: self.next_base().await,
            target_id,
            label,
        });
        self.append_typed_entry(entry).await
    }

    /// Append sanitized session name metadata.
    ///
    /// # Errors
    ///
    /// Returns storage errors from the backing session storage.
    pub async fn append_session_name(&self, name: impl AsRef<str>) -> Result<String, SessionError> {
        let sanitized = name
            .as_ref()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let entry = SessionTreeEntry::SessionInfo(crate::harness::types::SessionInfoEntry {
            base: self.next_base().await,
            name: Some(sanitized),
        });
        self.append_typed_entry(entry).await
    }

    /// Move the active leaf and optionally append a branch summary.
    ///
    /// # Errors
    ///
    /// Returns `not_found` for missing targets, or storage errors from the backing storage.
    pub async fn move_to(
        &self,
        entry_id: Option<String>,
        summary: Option<BranchSummaryDraft>,
    ) -> Result<Option<String>, SessionError> {
        if let Some(id) = entry_id.as_deref() {
            if self.storage.get_entry(id).await.is_none() {
                return Err(SessionError::new(
                    SessionErrorCode::NotFound,
                    format!("Entry {id} not found"),
                    None,
                ));
            }
        }
        self.storage.set_leaf_id(entry_id.clone()).await?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let entry = SessionTreeEntry::BranchSummary(BranchSummaryEntry {
            base: create_entry_base(self.storage.create_entry_id().await, entry_id.clone()),
            from_id: entry_id.unwrap_or_else(|| "root".to_string()),
            summary: summary.summary,
            details: summary.details,
            from_hook: summary.from_hook,
        });
        self.append_typed_entry(entry).await.map(Some)
    }

    async fn next_base(&self) -> SessionTreeEntryBase {
        create_entry_base(
            self.storage.create_entry_id().await,
            self.storage.get_leaf_id().await,
        )
    }
}

impl<TStorage> crate::harness::types::Session for Session<TStorage>
where
    TStorage: SessionStorage,
{
    fn get_metadata<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, SessionMetadata> {
        Box::pin(async move { self.get_metadata().await })
    }

    fn get_leaf_id<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, Option<String>> {
        Box::pin(async move { self.get_leaf_id().await })
    }

    fn get_entry<'a>(
        &'a self,
        id: &'a str,
    ) -> crate::harness::types::HarnessFuture<'a, Option<SessionTreeEntry>> {
        Box::pin(async move { self.get_entry(id).await })
    }

    fn get_entries<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move { self.get_entries().await })
    }

    fn get_branch<'a>(
        &'a self,
        from_id: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Vec<SessionTreeEntry>> {
        Box::pin(async move { self.get_branch(from_id).await })
    }

    fn build_context<'a>(&'a self) -> crate::harness::types::HarnessFuture<'a, SessionContext> {
        Box::pin(async move { self.build_context().await })
    }

    fn append_message<'a>(
        &'a self,
        message: AgentMessage,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_message(message).await })
    }

    fn append_model_change<'a>(
        &'a self,
        provider: String,
        model_id: String,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_model_change(provider, model_id).await })
    }

    fn append_thinking_level_change<'a>(
        &'a self,
        thinking_level: String,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_thinking_level_change(thinking_level).await })
    }

    fn append_active_tools_change<'a>(
        &'a self,
        active_tool_names: Vec<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_active_tools_change(active_tool_names).await })
    }

    fn append_compaction<'a>(
        &'a self,
        summary: String,
        first_kept_entry_id: String,
        tokens_before: u64,
        details: Option<Value>,
        from_hook: Option<bool>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            self.append_compaction(
                summary,
                first_kept_entry_id,
                tokens_before,
                details,
                from_hook,
            )
            .await
        })
    }

    fn append_custom_entry<'a>(
        &'a self,
        custom_type: String,
        data: Option<Value>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_custom_entry(custom_type, data).await })
    }

    fn append_custom_message_entry<'a>(
        &'a self,
        custom_type: String,
        content: CustomMessageContent,
        display: bool,
        details: Option<Value>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move {
            self.append_custom_message_entry(custom_type, content, display, details)
                .await
        })
    }

    fn append_label<'a>(
        &'a self,
        target_id: String,
        label: Option<String>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_label(target_id, label).await })
    }

    fn append_session_name<'a>(
        &'a self,
        name: String,
    ) -> crate::harness::types::HarnessFuture<'a, Result<String, SessionError>> {
        Box::pin(async move { self.append_session_name(name).await })
    }

    fn move_to<'a>(
        &'a self,
        entry_id: Option<String>,
        summary: Option<BranchSummaryDraft>,
    ) -> crate::harness::types::HarnessFuture<'a, Result<Option<String>, SessionError>> {
        Box::pin(async move { self.move_to(entry_id, summary).await })
    }
}

fn append_context_message(messages: &mut Vec<AgentMessage>, entry: &SessionTreeEntry) {
    match entry {
        SessionTreeEntry::Message(entry) => messages.push(entry.message.clone()),
        SessionTreeEntry::CustomMessage(entry) => messages.push(create_custom_message(
            &entry.custom_type,
            &entry.content,
            entry.display,
            entry.details.clone(),
            &entry.base.timestamp,
        )),
        SessionTreeEntry::BranchSummary(entry) if !entry.summary.is_empty() => messages.push(
            create_branch_summary_message(&entry.summary, &entry.from_id, &entry.base.timestamp),
        ),
        _ => {}
    }
}

fn assistant_model_ref(message: &AgentMessage) -> Option<SessionModelRef> {
    match message {
        AgentMessage::Llm(Message::Assistant(message)) => Some(SessionModelRef {
            provider: message.provider.clone(),
            model_id: message.model.clone(),
        }),
        AgentMessage::Custom(value) => Some(SessionModelRef {
            provider: value.get("provider")?.as_str()?.to_string(),
            model_id: value.get("model")?.as_str()?.to_string(),
        }),
        AgentMessage::Llm(_) => None,
    }
}

fn create_branch_summary_message(summary: &str, from_id: &str, timestamp: &str) -> AgentMessage {
    AgentMessage::Custom(json!({
        "role": "branchSummary",
        "summary": summary,
        "fromId": from_id,
        "timestamp": iso_to_unix_millis(timestamp).unwrap_or_default(),
    }))
}

fn create_compaction_summary_message(
    summary: &str,
    tokens_before: u64,
    timestamp: &str,
) -> AgentMessage {
    AgentMessage::Custom(json!({
        "role": "compactionSummary",
        "summary": summary,
        "tokensBefore": tokens_before,
        "timestamp": iso_to_unix_millis(timestamp).unwrap_or_default(),
    }))
}

fn create_custom_message(
    custom_type: &str,
    content: &CustomMessageContent,
    display: bool,
    details: Option<Value>,
    timestamp: &str,
) -> AgentMessage {
    AgentMessage::Custom(json!({
        "role": "custom",
        "customType": custom_type,
        "content": content,
        "display": display,
        "details": details,
        "timestamp": iso_to_unix_millis(timestamp).unwrap_or_default(),
    }))
}

#[allow(dead_code)]
fn _leaf_after(entry: &SessionTreeEntry) -> Option<String> {
    leaf_id_after_entry(entry)
}
