//! Conversation compaction helpers ported from Pi.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zedflow_ai::{
    AssistantContentBlock, Context, Message, Model, Models, SimpleStreamOptions, StopReason,
    TextContent, TextContentType, Usage, UserContentBlock, UserMessage, UserMessageContent,
    UserMessageRole,
};

use crate::harness::messages::convert_to_llm;
use crate::harness::session::session::build_session_context;
use crate::harness::types::{
    CompactionError, CompactionErrorCode, CompactionPreparation, CompactionSettings,
    FileOperations, Result, SessionTreeEntry,
};
use crate::types::{AgentMessage, ThinkingLevel};

use super::utils::{
    compute_file_lists, create_file_ops, extract_file_ops_from_message, format_file_operations,
    serialize_conversation,
};

/// Default compaction settings used by the harness.
pub const DEFAULT_COMPACTION_SETTINGS: CompactionSettings = CompactionSettings {
    enabled: true,
    reserve_tokens: 16_384,
    keep_recent_tokens: 20_000,
};

/// File-operation details stored on generated compaction entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionDetails {
    /// Files read in the compacted history.
    pub read_files: Vec<String>,
    /// Files modified in the compacted history.
    pub modified_files: Vec<String>,
}

/// Generated compaction data ready to be persisted as a compaction entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult<T = Value> {
    /// Summary text that replaces compacted history in future context.
    pub summary: String,
    /// Entry id where retained history starts.
    pub first_kept_entry_id: String,
    /// Estimated context tokens before compaction.
    pub tokens_before: u64,
    /// Optional implementation-specific details stored with the compaction entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<T>,
}

/// Estimated context-token usage for a message list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageEstimate {
    /// Estimated total context tokens.
    pub tokens: u64,
    /// Tokens reported by the most recent assistant usage block.
    pub usage_tokens: u64,
    /// Estimated tokens after the most recent assistant usage block.
    pub trailing_tokens: u64,
    /// Index of the message that provided usage, or `None` when none exists.
    pub last_usage_index: Option<usize>,
}

/// Cut point selected for compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutPointResult {
    /// Index of the first entry retained after compaction.
    pub first_kept_entry_index: usize,
    /// Index of the turn-start entry when the cut splits a turn.
    pub turn_start_index: Option<usize>,
    /// Whether the selected cut point splits an in-progress turn.
    pub is_split_turn: bool,
}

/// Calculate total context tokens from provider usage.
#[must_use]
pub fn calculate_context_tokens(usage: &Usage) -> u64 {
    usage.total_tokens.max(
        usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write),
    )
}

/// Return usage from the last valid assistant message in session entries.
#[must_use]
pub fn get_last_assistant_usage(entries: &[SessionTreeEntry]) -> Option<Usage> {
    entries.iter().rev().find_map(|entry| match entry {
        SessionTreeEntry::Message(entry) => get_assistant_usage(&entry.message).cloned(),
        _ => None,
    })
}

/// Estimate context tokens for messages using provider usage when available.
#[must_use]
pub fn estimate_context_tokens(messages: &[AgentMessage]) -> ContextUsageEstimate {
    let usage_info = messages
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, message)| get_assistant_usage(message).map(|usage| (index, usage)));

    let Some((last_usage_index, usage)) = usage_info else {
        let estimated = messages.iter().map(estimate_tokens).sum();
        return ContextUsageEstimate {
            tokens: estimated,
            usage_tokens: 0,
            trailing_tokens: estimated,
            last_usage_index: None,
        };
    };

    let usage_tokens = calculate_context_tokens(usage);
    let trailing_tokens = messages[last_usage_index + 1..]
        .iter()
        .map(estimate_tokens)
        .sum();

    ContextUsageEstimate {
        tokens: usage_tokens.saturating_add(trailing_tokens),
        usage_tokens,
        trailing_tokens,
        last_usage_index: Some(last_usage_index),
    }
}

/// Return whether context usage exceeds the configured compaction threshold.
#[must_use]
pub fn should_compact(
    context_tokens: u64,
    context_window: u64,
    settings: CompactionSettings,
) -> bool {
    settings.enabled && context_tokens > context_window.saturating_sub(settings.reserve_tokens)
}

/// Estimate token count for one message using a conservative character heuristic.
#[must_use]
pub fn estimate_tokens(message: &AgentMessage) -> u64 {
    let chars = match message {
        AgentMessage::Llm(Message::User(message)) => estimate_user_content_chars(&message.content),
        AgentMessage::Llm(Message::Assistant(message)) => message
            .content
            .iter()
            .map(|block| match block {
                AssistantContentBlock::Text(block) => block.text.len(),
                AssistantContentBlock::Thinking(block) => block.thinking.len(),
                AssistantContentBlock::ToolCall(block) => {
                    block.name.len() + safe_json_stringify(&block.arguments).len()
                }
            })
            .sum(),
        AgentMessage::Llm(Message::ToolResult(message)) => message
            .content
            .iter()
            .map(|block| match block {
                zedflow_ai::ToolResultContentBlock::Text(block) => block.text.len(),
                zedflow_ai::ToolResultContentBlock::Image(_) => ESTIMATED_IMAGE_CHARS,
            })
            .sum(),
        AgentMessage::Custom(value) => estimate_custom_message_chars(value),
    };
    chars.div_ceil(4) as u64
}

/// Find the user-visible message that starts the turn containing an entry.
#[must_use]
pub fn find_turn_start_index(
    entries: &[SessionTreeEntry],
    entry_index: usize,
    start_index: usize,
) -> Option<usize> {
    (start_index..=entry_index)
        .rev()
        .find(|index| match &entries[*index] {
            SessionTreeEntry::BranchSummary(_) | SessionTreeEntry::CustomMessage(_) => true,
            SessionTreeEntry::Message(entry) => matches!(
                message_role(&entry.message).as_deref(),
                Some("user" | "bashExecution")
            ),
            _ => false,
        })
}

/// Find the compaction cut point that keeps approximately the requested recent-token budget.
#[must_use]
pub fn find_cut_point(
    entries: &[SessionTreeEntry],
    start_index: usize,
    end_index: usize,
    keep_recent_tokens: u64,
) -> CutPointResult {
    let cut_points = find_valid_cut_points(entries, start_index, end_index);

    if cut_points.is_empty() {
        return CutPointResult {
            first_kept_entry_index: start_index,
            turn_start_index: None,
            is_split_turn: false,
        };
    }

    let mut accumulated_tokens = 0_u64;
    let mut cut_index = cut_points[0];

    for i in (start_index..end_index).rev() {
        let SessionTreeEntry::Message(entry) = &entries[i] else {
            continue;
        };
        accumulated_tokens = accumulated_tokens.saturating_add(estimate_tokens(&entry.message));
        if accumulated_tokens >= keep_recent_tokens {
            if let Some(point) = cut_points.iter().copied().find(|point| *point >= i) {
                cut_index = point;
            }
            break;
        }
    }

    while cut_index > start_index {
        match &entries[cut_index - 1] {
            SessionTreeEntry::Compaction(_) | SessionTreeEntry::Message(_) => break,
            _ => cut_index -= 1,
        }
    }

    let is_user_message = matches!(
        &entries[cut_index],
        SessionTreeEntry::Message(entry) if message_role(&entry.message).as_deref() == Some("user")
    );
    let turn_start_index = if is_user_message {
        None
    } else {
        find_turn_start_index(entries, cut_index, start_index)
    };

    CutPointResult {
        first_kept_entry_index: cut_index,
        turn_start_index,
        is_split_turn: !is_user_message && turn_start_index.is_some(),
    }
}

/// System prompt used for summarization calls.
pub const SUMMARIZATION_SYSTEM_PROMPT: &str = "You are a context summarization assistant. Your task is to read a conversation between a user and an AI assistant, then produce a structured summary following the exact format specified.\n\nDo NOT continue the conversation. Do NOT respond to any questions in the conversation. ONLY output the structured summary.";

const SUMMARIZATION_PROMPT: &str = "The messages above are a conversation to summarize. Create a structured context checkpoint summary that another LLM will use to continue the work.\n\nUse this EXACT format:\n\n## Goal\n[What is the user trying to accomplish? Can be multiple items if the session covers different tasks.]\n\n## Constraints & Preferences\n- [Any constraints, preferences, or requirements mentioned by user]\n- [Or \"(none)\" if none were mentioned]\n\n## Progress\n### Done\n- [x] [Completed tasks/changes]\n\n### In Progress\n- [ ] [Current work]\n\n### Blocked\n- [Issues preventing progress, if any]\n\n## Key Decisions\n- **[Decision]**: [Brief rationale]\n\n## Next Steps\n1. [Ordered list of what should happen next]\n\n## Critical Context\n- [Any data, examples, or references needed to continue]\n- [Or \"(none)\" if not applicable]\n\nKeep each section concise. Preserve exact file paths, function names, and error messages.";

const UPDATE_SUMMARIZATION_PROMPT: &str = "The messages above are NEW conversation messages to incorporate into the existing summary provided in <previous-summary> tags.\n\nUpdate the existing structured summary with new information. RULES:\n- PRESERVE all existing information from the previous summary\n- ADD new progress, decisions, and context from the new messages\n- UPDATE the Progress section: move items from \"In Progress\" to \"Done\" when completed\n- UPDATE \"Next Steps\" based on what was accomplished\n- PRESERVE exact file paths, function names, and error messages\n- If something is no longer relevant, you may remove it\n\nUse this EXACT format:\n\n## Goal\n[Preserve existing goals, add new ones if the task expanded]\n\n## Constraints & Preferences\n- [Preserve existing, add new ones discovered]\n\n## Progress\n### Done\n- [x] [Include previously done items AND newly completed tasks]\n\n### In Progress\n- [ ] [Current work - update based on progress]\n\n### Blocked\n- [Current blockers - remove if resolved]\n\n## Key Decisions\n- **[Decision]**: [Brief rationale] (preserve all previous, add new)\n\n## Next Steps\n1. [Update based on current state]\n\n## Critical Context\n- [Preserve important context, add new if needed]\n\nKeep each section concise. Preserve exact file paths, function names, and error messages.";

const TURN_PREFIX_SUMMARIZATION_PROMPT: &str = "This is the PREFIX of a turn that was too large to keep. The SUFFIX (recent work) is retained.\n\nSummarize the prefix to provide context for the retained suffix:\n\n## Original Request\n[What did the user ask for in this turn?]\n\n## Early Progress\n- [Key decisions and work done in the prefix]\n\n## Context for Suffix\n- [Information needed to understand the retained recent work]\n\nBe concise. Focus on what's needed to understand the kept suffix.";

/// Generate or update a conversation summary for compaction.
///
/// # Errors
///
/// Returns [`CompactionError`] when the model call aborts or reports an error.
pub fn generate_summary(
    current_messages: &[AgentMessage],
    models: &Models,
    model: &Model,
    reserve_tokens: u64,
    custom_instructions: Option<&str>,
    previous_summary: Option<&str>,
    thinking_level: Option<ThinkingLevel>,
) -> Result<String, CompactionError> {
    let max_tokens = capped_tokens(reserve_tokens.saturating_mul(4) / 5, model.max_tokens);
    let mut base_prompt = if previous_summary.is_some() {
        UPDATE_SUMMARIZATION_PROMPT.to_string()
    } else {
        SUMMARIZATION_PROMPT.to_string()
    };
    if let Some(custom_instructions) = custom_instructions {
        base_prompt.push_str("\n\nAdditional focus: ");
        base_prompt.push_str(custom_instructions);
    }

    let llm_messages = convert_to_llm(current_messages);
    let conversation_text = serialize_conversation(&llm_messages);
    let mut prompt_text = format!("<conversation>\n{conversation_text}\n</conversation>\n\n");
    if let Some(previous_summary) = previous_summary {
        prompt_text.push_str("<previous-summary>\n");
        prompt_text.push_str(previous_summary);
        prompt_text.push_str("\n</previous-summary>\n\n");
    }
    prompt_text.push_str(&base_prompt);

    complete_summary(
        models,
        model,
        prompt_text,
        max_tokens,
        "Summarization",
        thinking_level,
    )
}

/// Prepare session entries for compaction, or return `None` when compaction is not applicable.
///
/// # Errors
///
/// Returns [`CompactionError`] when the session path cannot provide a first kept entry id.
pub fn prepare_compaction(
    path_entries: &[SessionTreeEntry],
    settings: CompactionSettings,
) -> Result<Option<CompactionPreparation>, CompactionError> {
    if path_entries.is_empty()
        || matches!(path_entries.last(), Some(SessionTreeEntry::Compaction(_)))
    {
        return Ok(None);
    }

    let prev_compaction_index = path_entries
        .iter()
        .rposition(|entry| matches!(entry, SessionTreeEntry::Compaction(_)));

    let mut previous_summary = None;
    let mut boundary_start = 0;
    if let Some(index) = prev_compaction_index {
        if let SessionTreeEntry::Compaction(prev) = &path_entries[index] {
            previous_summary = Some(prev.summary.clone());
            boundary_start = path_entries
                .iter()
                .position(|entry| entry_id(entry) == prev.first_kept_entry_id)
                .unwrap_or(index + 1);
        }
    }
    let boundary_end = path_entries.len();
    let tokens_before =
        estimate_context_tokens(&build_session_context(path_entries).messages).tokens;

    let cut_point = find_cut_point(
        path_entries,
        boundary_start,
        boundary_end,
        settings.keep_recent_tokens,
    );
    let first_kept_entry = path_entries
        .get(cut_point.first_kept_entry_index)
        .ok_or_else(|| {
            compaction_error(
                CompactionErrorCode::InvalidSession,
                "First kept entry has no UUID - session may need migration",
            )
        })?;
    let first_kept_entry_id = entry_id(first_kept_entry);
    if first_kept_entry_id.is_empty() {
        return Err(compaction_error(
            CompactionErrorCode::InvalidSession,
            "First kept entry has no UUID - session may need migration",
        ));
    }

    let history_end = if cut_point.is_split_turn {
        cut_point
            .turn_start_index
            .unwrap_or(cut_point.first_kept_entry_index)
    } else {
        cut_point.first_kept_entry_index
    };
    let messages_to_summarize = path_entries[boundary_start..history_end]
        .iter()
        .filter_map(get_message_from_entry_for_compaction)
        .collect::<Vec<_>>();

    let turn_prefix_messages = if cut_point.is_split_turn {
        let turn_start = cut_point
            .turn_start_index
            .unwrap_or(cut_point.first_kept_entry_index);
        path_entries[turn_start..cut_point.first_kept_entry_index]
            .iter()
            .filter_map(get_message_from_entry_for_compaction)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut file_ops =
        extract_file_operations(&messages_to_summarize, path_entries, prev_compaction_index);
    if cut_point.is_split_turn {
        for message in &turn_prefix_messages {
            extract_file_ops_from_message(message, &mut file_ops);
        }
    }

    Ok(Some(CompactionPreparation {
        first_kept_entry_id,
        messages_to_summarize,
        turn_prefix_messages,
        is_split_turn: cut_point.is_split_turn,
        tokens_before,
        previous_summary,
        file_ops,
        settings,
    }))
}

/// Generate compaction summary data from prepared session history.
///
/// # Errors
///
/// Returns [`CompactionError`] when model summarization aborts or fails.
pub fn compact(
    preparation: &CompactionPreparation,
    models: &Models,
    model: &Model,
    custom_instructions: Option<&str>,
    thinking_level: Option<ThinkingLevel>,
) -> Result<CompactionResult, CompactionError> {
    if preparation.first_kept_entry_id.is_empty() {
        return Err(compaction_error(
            CompactionErrorCode::InvalidSession,
            "First kept entry has no UUID - session may need migration",
        ));
    }

    let mut summary = if preparation.is_split_turn && !preparation.turn_prefix_messages.is_empty() {
        let history = if preparation.messages_to_summarize.is_empty() {
            "No prior history.".to_string()
        } else {
            generate_summary(
                &preparation.messages_to_summarize,
                models,
                model,
                preparation.settings.reserve_tokens,
                custom_instructions,
                preparation.previous_summary.as_deref(),
                thinking_level,
            )?
        };
        let turn_prefix = generate_turn_prefix_summary(
            &preparation.turn_prefix_messages,
            models,
            model,
            preparation.settings.reserve_tokens,
            thinking_level,
        )?;
        format!("{history}\n\n---\n\n**Turn Context (split turn):**\n\n{turn_prefix}")
    } else {
        generate_summary(
            &preparation.messages_to_summarize,
            models,
            model,
            preparation.settings.reserve_tokens,
            custom_instructions,
            preparation.previous_summary.as_deref(),
            thinking_level,
        )?
    };

    let file_lists = compute_file_lists(&preparation.file_ops);
    summary.push_str(&format_file_operations(
        &file_lists.read_files,
        &file_lists.modified_files,
    ));

    Ok(CompactionResult {
        summary,
        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
        tokens_before: preparation.tokens_before,
        details: Some(json!(CompactionDetails {
            read_files: file_lists.read_files,
            modified_files: file_lists.modified_files,
        })),
    })
}

pub(crate) fn get_message_from_entry(entry: &SessionTreeEntry) -> Option<AgentMessage> {
    match entry {
        SessionTreeEntry::Message(entry) => Some(entry.message.clone()),
        SessionTreeEntry::CustomMessage(entry) => Some(AgentMessage::Custom(json!({
            "role": "custom",
            "customType": entry.custom_type,
            "content": entry.content,
            "display": entry.display,
            "details": entry.details,
            "timestamp": iso_to_unix_millis(&entry.base.timestamp).unwrap_or_default(),
        }))),
        SessionTreeEntry::BranchSummary(entry) => Some(AgentMessage::Custom(json!({
            "role": "branchSummary",
            "summary": entry.summary,
            "fromId": entry.from_id,
            "timestamp": iso_to_unix_millis(&entry.base.timestamp).unwrap_or_default(),
        }))),
        SessionTreeEntry::Compaction(entry) => Some(AgentMessage::Custom(json!({
            "role": "compactionSummary",
            "summary": entry.summary,
            "tokensBefore": entry.tokens_before,
            "timestamp": iso_to_unix_millis(&entry.base.timestamp).unwrap_or_default(),
        }))),
        _ => None,
    }
}

pub(crate) fn get_message_from_entry_for_compaction(
    entry: &SessionTreeEntry,
) -> Option<AgentMessage> {
    if matches!(entry, SessionTreeEntry::Compaction(_)) {
        None
    } else {
        get_message_from_entry(entry)
    }
}

fn get_assistant_usage(message: &AgentMessage) -> Option<&Usage> {
    let AgentMessage::Llm(Message::Assistant(message)) = message else {
        return None;
    };
    (!matches!(message.stop_reason, StopReason::Aborted | StopReason::Error)
        && calculate_context_tokens(&message.usage) > 0)
        .then_some(&message.usage)
}

fn extract_file_operations(
    messages: &[AgentMessage],
    entries: &[SessionTreeEntry],
    prev_compaction_index: Option<usize>,
) -> FileOperations {
    let mut file_ops = create_file_ops();
    if let Some(index) = prev_compaction_index {
        if let SessionTreeEntry::Compaction(prev) = &entries[index] {
            if !prev.from_hook.unwrap_or(false) {
                merge_details(&mut file_ops, prev.details.as_ref());
            }
        }
    }
    for message in messages {
        extract_file_ops_from_message(message, &mut file_ops);
    }
    file_ops
}

pub(crate) fn merge_details(file_ops: &mut FileOperations, details: Option<&Value>) {
    let Some(details) = details else {
        return;
    };
    if let Some(read_files) = details.get("readFiles").and_then(|value| value.as_array()) {
        for file in read_files.iter().filter_map(|value| value.as_str()) {
            file_ops.read.insert(file.to_string());
        }
    }
    if let Some(modified_files) = details
        .get("modifiedFiles")
        .and_then(|value| value.as_array())
    {
        for file in modified_files.iter().filter_map(|value| value.as_str()) {
            file_ops.edited.insert(file.to_string());
        }
    }
}

fn find_valid_cut_points(
    entries: &[SessionTreeEntry],
    start_index: usize,
    end_index: usize,
) -> Vec<usize> {
    let mut cut_points = Vec::new();
    for (i, entry) in entries.iter().enumerate().take(end_index).skip(start_index) {
        match entry {
            SessionTreeEntry::Message(entry) => {
                if !matches!(message_role(&entry.message).as_deref(), Some("toolResult")) {
                    cut_points.push(i);
                }
            }
            SessionTreeEntry::BranchSummary(_) | SessionTreeEntry::CustomMessage(_) => {
                cut_points.push(i)
            }
            _ => {}
        }
    }
    cut_points
}

fn message_role(message: &AgentMessage) -> Option<String> {
    match message {
        AgentMessage::Llm(Message::User(_)) => Some("user".to_string()),
        AgentMessage::Llm(Message::Assistant(_)) => Some("assistant".to_string()),
        AgentMessage::Llm(Message::ToolResult(_)) => Some("toolResult".to_string()),
        AgentMessage::Custom(value) => value.get("role")?.as_str().map(ToOwned::to_owned),
    }
}

const ESTIMATED_IMAGE_CHARS: usize = 4_800;

fn estimate_user_content_chars(content: &UserMessageContent) -> usize {
    match content {
        UserMessageContent::Text(text) => text.len(),
        UserMessageContent::Blocks(blocks) => blocks
            .iter()
            .map(|block| match block {
                UserContentBlock::Text(block) => block.text.len(),
                UserContentBlock::Image(_) => ESTIMATED_IMAGE_CHARS,
            })
            .sum(),
    }
}

fn estimate_custom_message_chars(value: &Value) -> usize {
    match value.get("role").and_then(|role| role.as_str()) {
        Some("custom" | "toolResult") => estimate_value_content_chars(value.get("content")),
        Some("bashExecution") => {
            string_len(value, "command").saturating_add(string_len(value, "output"))
        }
        Some("branchSummary" | "compactionSummary") => string_len(value, "summary"),
        _ => 0,
    }
}

fn estimate_value_content_chars(content: Option<&Value>) -> usize {
    match content {
        Some(Value::String(text)) => text.len(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .map(
                |block| match block.get("type").and_then(|value| value.as_str()) {
                    Some("text") => string_len(block, "text"),
                    Some("image") => ESTIMATED_IMAGE_CHARS,
                    _ => 0,
                },
            )
            .sum(),
        _ => 0,
    }
}

fn string_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map_or(0, str::len)
}

fn safe_json_stringify(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[unserializable]".to_string())
}

fn generate_turn_prefix_summary(
    messages: &[AgentMessage],
    models: &Models,
    model: &Model,
    reserve_tokens: u64,
    thinking_level: Option<ThinkingLevel>,
) -> Result<String, CompactionError> {
    let max_tokens = capped_tokens(reserve_tokens / 2, model.max_tokens);
    let conversation_text = serialize_conversation(&convert_to_llm(messages));
    complete_summary(
        models,
        model,
        format!(
            "<conversation>\n{conversation_text}\n</conversation>\n\n{TURN_PREFIX_SUMMARIZATION_PROMPT}"
        ),
        max_tokens,
        "Turn prefix summarization",
        thinking_level,
    )
}

pub(crate) fn complete_summary(
    models: &Models,
    model: &Model,
    prompt_text: String,
    max_tokens: u32,
    label: &str,
    thinking_level: Option<ThinkingLevel>,
) -> Result<String, CompactionError> {
    let mut options = SimpleStreamOptions::default();
    options.stream.max_tokens = Some(max_tokens);
    options.reasoning = reasoning_option(model, thinking_level);

    let response = models.complete_simple(
        model,
        &Context {
            system_prompt: Some(SUMMARIZATION_SYSTEM_PROMPT.to_string()),
            messages: vec![summary_user_message(prompt_text)],
            tools: None,
        },
        Some(&options),
    );

    match response.stop_reason {
        StopReason::Aborted => Err(compaction_error(
            CompactionErrorCode::Aborted,
            response
                .error_message
                .unwrap_or_else(|| format!("{label} aborted")),
        )),
        StopReason::Error => Err(compaction_error(
            CompactionErrorCode::SummarizationFailed,
            format!(
                "{label} failed: {}",
                response
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ),
        )),
        _ => Ok(response
            .content
            .iter()
            .filter_map(|block| match block {
                AssistantContentBlock::Text(block) => Some(block.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")),
    }
}

pub(crate) fn summary_user_message(prompt_text: String) -> Message {
    Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Blocks(vec![UserContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: prompt_text,
            text_signature: None,
        })]),
        timestamp: now_millis(),
    })
}

pub(crate) fn reasoning_option(
    model: &Model,
    thinking_level: Option<ThinkingLevel>,
) -> Option<zedflow_ai::ThinkingLevel> {
    if !model.reasoning {
        return None;
    }
    match thinking_level.unwrap_or(ThinkingLevel::Off) {
        ThinkingLevel::Off => None,
        ThinkingLevel::Minimal => Some(zedflow_ai::ThinkingLevel::Minimal),
        ThinkingLevel::Low => Some(zedflow_ai::ThinkingLevel::Low),
        ThinkingLevel::Medium => Some(zedflow_ai::ThinkingLevel::Medium),
        ThinkingLevel::High => Some(zedflow_ai::ThinkingLevel::High),
        ThinkingLevel::XHigh => Some(zedflow_ai::ThinkingLevel::XHigh),
    }
}

fn capped_tokens(budget: u64, model_max_tokens: u64) -> u32 {
    let tokens = if model_max_tokens > 0 {
        budget.min(model_max_tokens)
    } else {
        budget
    };
    u32::try_from(tokens).unwrap_or(u32::MAX)
}

fn compaction_error(code: CompactionErrorCode, message: impl Into<String>) -> CompactionError {
    CompactionError::new(code, message, None)
}

pub(crate) fn entry_id(entry: &SessionTreeEntry) -> String {
    match entry {
        SessionTreeEntry::Message(entry) => entry.base.id.clone(),
        SessionTreeEntry::ThinkingLevelChange(entry) => entry.base.id.clone(),
        SessionTreeEntry::ModelChange(entry) => entry.base.id.clone(),
        SessionTreeEntry::ActiveToolsChange(entry) => entry.base.id.clone(),
        SessionTreeEntry::Compaction(entry) => entry.base.id.clone(),
        SessionTreeEntry::BranchSummary(entry) => entry.base.id.clone(),
        SessionTreeEntry::Custom(entry) => entry.base.id.clone(),
        SessionTreeEntry::CustomMessage(entry) => entry.base.id.clone(),
        SessionTreeEntry::Label(entry) => entry.base.id.clone(),
        SessionTreeEntry::SessionInfo(entry) => entry.base.id.clone(),
        SessionTreeEntry::Leaf(entry) => entry.base.id.clone(),
    }
}

pub(crate) fn parent_id(entry: &SessionTreeEntry) -> Option<String> {
    match entry {
        SessionTreeEntry::Message(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::ThinkingLevelChange(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::ModelChange(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::ActiveToolsChange(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::Compaction(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::BranchSummary(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::Custom(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::CustomMessage(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::Label(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::SessionInfo(entry) => entry.base.parent_id.clone(),
        SessionTreeEntry::Leaf(entry) => entry.base.parent_id.clone(),
    }
}

fn iso_to_unix_millis(input: &str) -> Option<i64> {
    let bytes = input.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let year = parse_digits(input.get(0..4)?)?;
    let month = parse_digits(input.get(5..7)?)?;
    let day = parse_digits(input.get(8..10)?)?;
    let hour = parse_digits(input.get(11..13)?)?;
    let minute = parse_digits(input.get(14..16)?)?;
    let second = parse_digits(input.get(17..19)?)?;
    if input.get(4..5)? != "-"
        || input.get(7..8)? != "-"
        || !matches!(input.get(10..11)?, "T" | "t" | " ")
        || input.get(13..14)? != ":"
        || input.get(16..17)? != ":"
    {
        return None;
    }

    let mut index = 19;
    let mut millis = 0_i64;
    if input.get(index..index + 1) == Some(".") {
        index += 1;
        let start = index;
        while index < input.len() && input.as_bytes()[index].is_ascii_digit() {
            index += 1;
        }
        let fraction = &input[start..index.min(start + 3)];
        if !fraction.is_empty() {
            let mut value = parse_digits(fraction)?;
            for _ in fraction.len()..3 {
                value *= 10;
            }
            millis = value as i64;
        }
    }

    let offset_minutes = match input.get(index..index + 1)? {
        "Z" | "z" => 0,
        "+" | "-" => {
            let sign = if input.get(index..index + 1)? == "+" {
                1
            } else {
                -1
            };
            let h = parse_digits(input.get(index + 1..index + 3)?)? as i64;
            let m = parse_digits(input.get(index + 4..index + 6)?)? as i64;
            if input.get(index + 3..index + 4)? != ":" {
                return None;
            }
            sign * (h * 60 + m)
        }
        _ => return None,
    };

    let days = days_from_civil(year as i64, month as i64, day as i64)?;
    let seconds = days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64
        - offset_minutes * 60;
    Some(seconds * 1_000 + millis)
}

fn parse_digits(value: &str) -> Option<u32> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())?
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
