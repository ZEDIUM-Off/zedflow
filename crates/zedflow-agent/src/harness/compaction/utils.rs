//! Shared helpers for compaction and branch summarization.

use std::collections::BTreeSet;

use zedflow_ai::{
    AssistantContentBlock, Message, ToolResultContentBlock, UserContentBlock, UserMessageContent,
};

use crate::harness::types::FileOperations;
use crate::types::AgentMessage;

const TOOL_RESULT_MAX_CHARS: usize = 2_000;

/// Sorted file lists derived from accumulated file operations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLists {
    /// Files read but not modified.
    pub read_files: Vec<String>,
    /// Files modified through write or edit operations.
    pub modified_files: Vec<String>,
}

/// Create an empty file-operation accumulator.
#[must_use]
pub fn create_file_ops() -> FileOperations {
    FileOperations::default()
}

/// Add file operations from assistant tool calls to an accumulator.
pub fn extract_file_ops_from_message(message: &AgentMessage, file_ops: &mut FileOperations) {
    let AgentMessage::Llm(Message::Assistant(message)) = message else {
        return;
    };

    for block in &message.content {
        let AssistantContentBlock::ToolCall(call) = block else {
            continue;
        };
        let Some(path) = call.arguments.get("path").and_then(|value| value.as_str()) else {
            continue;
        };

        match call.name.as_str() {
            "read" => {
                file_ops.read.insert(path.to_string());
            }
            "write" => {
                file_ops.written.insert(path.to_string());
            }
            "edit" => {
                file_ops.edited.insert(path.to_string());
            }
            _ => {}
        }
    }
}

/// Compute sorted read-only and modified file lists from accumulated operations.
#[must_use]
pub fn compute_file_lists(file_ops: &FileOperations) -> FileLists {
    let modified: BTreeSet<String> = file_ops
        .edited
        .iter()
        .chain(file_ops.written.iter())
        .cloned()
        .collect();
    let read_files = file_ops
        .read
        .iter()
        .filter(|path| !modified.contains(*path))
        .cloned()
        .collect();

    FileLists {
        read_files,
        modified_files: modified.into_iter().collect(),
    }
}

/// Format file lists as summary metadata tags.
#[must_use]
pub fn format_file_operations(read_files: &[String], modified_files: &[String]) -> String {
    let mut sections = Vec::new();
    if !read_files.is_empty() {
        sections.push(format!(
            "<read-files>\n{}\n</read-files>",
            read_files.join("\n")
        ));
    }
    if !modified_files.is_empty() {
        sections.push(format!(
            "<modified-files>\n{}\n</modified-files>",
            modified_files.join("\n")
        ));
    }
    if sections.is_empty() {
        String::new()
    } else {
        format!("\n\n{}", sections.join("\n\n"))
    }
}

/// Serialize LLM messages to plain text for summarization prompts.
#[must_use]
pub fn serialize_conversation(messages: &[Message]) -> String {
    let mut parts = Vec::new();

    for msg in messages {
        match msg {
            Message::User(msg) => {
                let content = user_content_text(&msg.content);
                if !content.is_empty() {
                    parts.push(format!("[User]: {content}"));
                }
            }
            Message::Assistant(msg) => {
                let mut text_parts = Vec::new();
                let mut thinking_parts = Vec::new();
                let mut tool_calls = Vec::new();

                for block in &msg.content {
                    match block {
                        AssistantContentBlock::Text(block) => text_parts.push(block.text.clone()),
                        AssistantContentBlock::Thinking(block) => {
                            thinking_parts.push(block.thinking.clone());
                        }
                        AssistantContentBlock::ToolCall(block) => {
                            let args = block
                                .arguments
                                .iter()
                                .map(|(key, value)| format!("{key}={}", safe_json_stringify(value)))
                                .collect::<Vec<_>>()
                                .join(", ");
                            tool_calls.push(format!("{}({args})", block.name));
                        }
                    }
                }

                if !thinking_parts.is_empty() {
                    parts.push(format!(
                        "[Assistant thinking]: {}",
                        thinking_parts.join("\n")
                    ));
                }
                if !text_parts.is_empty() {
                    parts.push(format!("[Assistant]: {}", text_parts.join("\n")));
                }
                if !tool_calls.is_empty() {
                    parts.push(format!("[Assistant tool calls]: {}", tool_calls.join("; ")));
                }
            }
            Message::ToolResult(msg) => {
                let content = msg
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ToolResultContentBlock::Text(block) => Some(block.text.as_str()),
                        ToolResultContentBlock::Image(_) => None,
                    })
                    .collect::<String>();
                if !content.is_empty() {
                    parts.push(format!(
                        "[Tool result]: {}",
                        truncate_for_summary(&content, TOOL_RESULT_MAX_CHARS)
                    ));
                }
            }
        }
    }

    parts.join("\n\n")
}

fn user_content_text(content: &UserMessageContent) -> String {
    match content {
        UserMessageContent::Text(text) => text.clone(),
        UserMessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                UserContentBlock::Text(block) => Some(block.text.as_str()),
                UserContentBlock::Image(_) => None,
            })
            .collect(),
    }
}

fn safe_json_stringify(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[unserializable]".to_string())
}

fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    let mut end = 0;
    for (idx, _) in text.char_indices() {
        if idx > max_chars {
            break;
        }
        end = idx;
    }
    if end == 0 {
        end = max_chars.min(text.len());
    }
    let truncated_chars = text.len() - end;
    format!(
        "{}\n\n[... {truncated_chars} more characters truncated]",
        &text[..end]
    )
}
