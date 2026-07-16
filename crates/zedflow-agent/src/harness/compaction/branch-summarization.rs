//! Branch summarization helpers ported from Pi.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zedflow_ai::{
    AbortSignal, AssistantContentBlock, Context, Model, Models, SimpleStreamOptions, StopReason,
};

use crate::harness::messages::convert_to_llm;
use crate::harness::types::{
    BranchSummaryError, BranchSummaryErrorCode, BranchSummaryResult, FileOperations, Result,
    Session, SessionError, SessionErrorCode, SessionTreeEntry,
};
use crate::types::AgentMessage;

use super::compaction::{
    SUMMARIZATION_SYSTEM_PROMPT, entry_id, estimate_tokens, get_message_from_entry, merge_details,
    parent_id, summary_user_message,
};
use super::utils::{
    compute_file_lists, create_file_ops, extract_file_ops_from_message, format_file_operations,
    serialize_conversation,
};

/// File-operation details stored on generated branch summary entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummaryDetails {
    /// Files read while exploring the summarized branch.
    pub read_files: Vec<String>,
    /// Files modified while exploring the summarized branch.
    pub modified_files: Vec<String>,
}

/// Prepared branch content for summarization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchPreparation {
    /// Messages selected for the branch summary.
    pub messages: Vec<AgentMessage>,
    /// File operations extracted from the branch.
    pub file_ops: FileOperations,
    /// Estimated token count for selected messages.
    pub total_tokens: u64,
}

/// Entries selected for branch summarization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectEntriesResult {
    /// Entries to summarize in chronological order.
    pub entries: Vec<SessionTreeEntry>,
    /// Deepest common ancestor between the previous leaf and target entry.
    pub common_ancestor_id: Option<String>,
}

/// Options for generating a branch summary.
pub struct GenerateBranchSummaryOptions<'a> {
    /// Provider collection the summarization request goes through; owns auth resolution.
    pub models: &'a Models,
    /// Model used for summarization.
    pub model: &'a Model,
    /// Optional abort signal for the summarization request.
    pub signal: Option<AbortSignal>,
    /// Optional instructions appended to or replacing the default prompt.
    pub custom_instructions: Option<&'a str>,
    /// Replace the default prompt with custom instructions instead of appending them.
    pub replace_instructions: bool,
    /// Tokens reserved for prompt and model output. Defaults to 16384.
    pub reserve_tokens: Option<u64>,
}

/// Collect entries that should be summarized before navigating to a different session tree entry.
///
/// # Errors
///
/// Returns [`SessionError`] when the previous leaf path references a missing entry.
pub async fn collect_entries_for_branch_summary(
    session: &dyn Session,
    old_leaf_id: Option<&str>,
    target_id: &str,
) -> std::result::Result<CollectEntriesResult, SessionError> {
    let Some(old_leaf_id) = old_leaf_id else {
        return Ok(CollectEntriesResult {
            entries: Vec::new(),
            common_ancestor_id: None,
        });
    };

    let old_path = session
        .get_branch(Some(old_leaf_id.to_string()))
        .await
        .into_iter()
        .map(|entry| entry_id(&entry))
        .collect::<HashSet<_>>();
    let target_path = session.get_branch(Some(target_id.to_string())).await;

    let common_ancestor_id = target_path
        .iter()
        .rev()
        .map(entry_id)
        .find(|id| old_path.contains(id));

    let mut entries = Vec::new();
    let mut current = Some(old_leaf_id.to_string());
    while let Some(id) = current {
        if Some(&id) == common_ancestor_id.as_ref() {
            break;
        }
        let entry = session.get_entry(&id).await.ok_or_else(|| {
            SessionError::new(
                SessionErrorCode::InvalidSession,
                format!("Entry {id} not found"),
                None,
            )
        })?;
        current = parent_id(&entry);
        entries.push(entry);
    }
    entries.reverse();

    Ok(CollectEntriesResult {
        entries,
        common_ancestor_id,
    })
}

/// Prepare branch entries for summarization within an optional token budget.
#[must_use]
pub fn prepare_branch_entries(
    entries: &[SessionTreeEntry],
    token_budget: u64,
) -> BranchPreparation {
    let mut messages = Vec::new();
    let mut file_ops = create_file_ops();
    let mut total_tokens = 0_u64;

    for entry in entries {
        if let SessionTreeEntry::BranchSummary(entry) = entry {
            if !entry.from_hook.unwrap_or(false) {
                merge_details(&mut file_ops, entry.details.as_ref());
            }
        }
    }

    for entry in entries.iter().rev() {
        let Some(message) = get_message_from_entry(entry) else {
            continue;
        };
        if matches!(
            &message,
            AgentMessage::Llm(zedflow_ai::Message::ToolResult(_))
        ) {
            continue;
        }
        extract_file_ops_from_message(&message, &mut file_ops);

        let tokens = estimate_tokens(&message);
        if token_budget > 0 && total_tokens.saturating_add(tokens) > token_budget {
            if matches!(
                entry,
                SessionTreeEntry::Compaction(_) | SessionTreeEntry::BranchSummary(_)
            ) && total_tokens < token_budget.saturating_mul(9) / 10
            {
                total_tokens = total_tokens.saturating_add(tokens);
                messages.insert(0, message);
            }
            break;
        }

        total_tokens = total_tokens.saturating_add(tokens);
        messages.insert(0, message);
    }

    BranchPreparation {
        messages,
        file_ops,
        total_tokens,
    }
}

const BRANCH_SUMMARY_PREAMBLE: &str = "The user explored a different conversation branch before returning here.\nSummary of that exploration:\n\n";

const BRANCH_SUMMARY_PROMPT: &str = "Create a structured summary of this conversation branch for context when returning later.\n\nUse this EXACT format:\n\n## Goal\n[What was the user trying to accomplish in this branch?]\n\n## Constraints & Preferences\n- [Any constraints, preferences, or requirements mentioned]\n- [Or \"(none)\" if none were mentioned]\n\n## Progress\n### Done\n- [x] [Completed tasks/changes]\n\n### In Progress\n- [ ] [Work that was started but not finished]\n\n### Blocked\n- [Issues preventing progress, if any]\n\n## Key Decisions\n- **[Decision]**: [Brief rationale]\n\n## Next Steps\n1. [What should happen next to continue this work]\n\nKeep each section concise. Preserve exact file paths, function names, and error messages.";

/// Generate a summary for abandoned branch entries.
///
/// # Errors
///
/// Returns [`BranchSummaryError`] when the model call aborts or reports an error.
pub fn generate_branch_summary(
    entries: &[SessionTreeEntry],
    options: GenerateBranchSummaryOptions<'_>,
) -> Result<BranchSummaryResult, BranchSummaryError> {
    let context_window = if options.model.context_window > 0 {
        options.model.context_window
    } else {
        128_000
    };
    let reserve_tokens = options.reserve_tokens.unwrap_or(16_384);
    let token_budget = context_window.saturating_sub(reserve_tokens);
    let BranchPreparation {
        messages, file_ops, ..
    } = prepare_branch_entries(entries, token_budget);

    if messages.is_empty() {
        return Ok(BranchSummaryResult {
            summary: "No content to summarize".to_string(),
            read_files: Vec::new(),
            modified_files: Vec::new(),
        });
    }

    let conversation_text = serialize_conversation(&convert_to_llm(&messages));
    let instructions = match (options.replace_instructions, options.custom_instructions) {
        (true, Some(custom)) => custom.to_string(),
        (false, Some(custom)) => format!("{BRANCH_SUMMARY_PROMPT}\n\nAdditional focus: {custom}"),
        _ => BRANCH_SUMMARY_PROMPT.to_string(),
    };
    let prompt_text =
        format!("<conversation>\n{conversation_text}\n</conversation>\n\n{instructions}");

    let mut stream_options = SimpleStreamOptions::default();
    stream_options.stream.max_tokens = Some(2_048);
    stream_options.stream.signal = options.signal;

    let response = options.models.complete_simple(
        options.model,
        &Context {
            system_prompt: Some(SUMMARIZATION_SYSTEM_PROMPT.to_string()),
            messages: vec![summary_user_message(prompt_text)],
            tools: None,
        },
        Some(&stream_options),
    );

    if response.stop_reason == StopReason::Aborted {
        return Err(branch_error(
            BranchSummaryErrorCode::Aborted,
            response
                .error_message
                .unwrap_or_else(|| "Branch summary aborted".to_string()),
        ));
    }
    if response.stop_reason == StopReason::Error {
        return Err(branch_error(
            BranchSummaryErrorCode::SummarizationFailed,
            format!(
                "Branch summary failed: {}",
                response
                    .error_message
                    .unwrap_or_else(|| "Unknown error".to_string())
            ),
        ));
    }

    let mut summary = response
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(block) => Some(block.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    summary.insert_str(0, BRANCH_SUMMARY_PREAMBLE);

    let file_lists = compute_file_lists(&file_ops);
    summary.push_str(&format_file_operations(
        &file_lists.read_files,
        &file_lists.modified_files,
    ));

    Ok(BranchSummaryResult {
        summary: if summary.is_empty() {
            "No summary generated".to_string()
        } else {
            summary
        },
        read_files: file_lists.read_files,
        modified_files: file_lists.modified_files,
    })
}

fn branch_error(code: BranchSummaryErrorCode, message: impl Into<String>) -> BranchSummaryError {
    BranchSummaryError::new(code, message, None)
}

/// Convert branch summary details into a session details value.
#[must_use]
pub fn branch_summary_details_value(read_files: Vec<String>, modified_files: Vec<String>) -> Value {
    serde_json::json!(BranchSummaryDetails {
        read_files,
        modified_files,
    })
}
