use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{
    AssistantContentBlock, AssistantMessage, Message, Model, ModelInput, StopReason, TextContent,
    TextContentType, ThinkingContent, ToolCall, ToolResultContentBlock, ToolResultMessage,
    ToolResultMessageRole, UserContentBlock, UserMessageContent,
};

const NON_VISION_USER_IMAGE_PLACEHOLDER: &str = "(image omitted: model does not support images)";
const NON_VISION_TOOL_IMAGE_PLACEHOLDER: &str =
    "(tool image omitted: model does not support images)";

/// Callback used to normalize tool-call IDs for the destination model.
pub type ToolCallIdNormalizer<'a> =
    dyn Fn(&str, &Model, &AssistantMessage) -> String + Send + Sync + 'a;

fn text(text: impl Into<String>) -> TextContent {
    TextContent {
        content_type: TextContentType::Text,
        text: text.into(),
        text_signature: None,
    }
}

fn replace_user_images(content: &[UserContentBlock], placeholder: &str) -> Vec<UserContentBlock> {
    let mut result = Vec::with_capacity(content.len());
    let mut previous_was_placeholder = false;
    for block in content {
        match block {
            UserContentBlock::Image(_) => {
                if !previous_was_placeholder {
                    result.push(UserContentBlock::Text(text(placeholder)));
                }
                previous_was_placeholder = true;
            }
            UserContentBlock::Text(value) => {
                result.push(block.clone());
                previous_was_placeholder = value.text == placeholder;
            }
        }
    }
    result
}

fn replace_tool_images(
    content: &[ToolResultContentBlock],
    placeholder: &str,
) -> Vec<ToolResultContentBlock> {
    let mut result = Vec::with_capacity(content.len());
    let mut previous_was_placeholder = false;
    for block in content {
        match block {
            ToolResultContentBlock::Image(_) => {
                if !previous_was_placeholder {
                    result.push(ToolResultContentBlock::Text(text(placeholder)));
                }
                previous_was_placeholder = true;
            }
            ToolResultContentBlock::Text(value) => {
                result.push(block.clone());
                previous_was_placeholder = value.text == placeholder;
            }
        }
    }
    result
}

fn downgrade_unsupported_images(messages: &[Message], model: &Model) -> Vec<Message> {
    if model.input.contains(&ModelInput::Image) {
        return messages.to_vec();
    }
    messages
        .iter()
        .map(|message| match message {
            Message::User(user) => {
                let mut user = user.clone();
                if let UserMessageContent::Blocks(content) = &user.content {
                    user.content = UserMessageContent::Blocks(replace_user_images(
                        content,
                        NON_VISION_USER_IMAGE_PLACEHOLDER,
                    ));
                }
                Message::User(user)
            }
            Message::ToolResult(tool_result) => {
                let mut tool_result = tool_result.clone();
                tool_result.content =
                    replace_tool_images(&tool_result.content, NON_VISION_TOOL_IMAGE_PLACEHOLDER);
                Message::ToolResult(tool_result)
            }
            Message::Assistant(_) => message.clone(),
        })
        .collect()
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn insert_synthetic_tool_results(
    result: &mut Vec<Message>,
    pending_tool_calls: &mut Vec<ToolCall>,
    existing_tool_result_ids: &mut HashSet<String>,
) {
    for tool_call in pending_tool_calls.drain(..) {
        if !existing_tool_result_ids.contains(&tool_call.id) {
            result.push(Message::ToolResult(ToolResultMessage {
                role: ToolResultMessageRole::ToolResult,
                tool_call_id: tool_call.id,
                tool_name: tool_call.name,
                content: vec![ToolResultContentBlock::Text(text("No result provided"))],
                details: None,
                is_error: true,
                timestamp: unix_millis(),
            }));
        }
    }
    existing_tool_result_ids.clear();
}

/// Normalizes replayed messages for the destination model using Pi's compatibility rules.
#[must_use]
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    normalize_tool_call_id: Option<&ToolCallIdNormalizer<'_>>,
) -> Vec<Message> {
    let mut tool_call_id_map: HashMap<String, String> = HashMap::new();
    let transformed = downgrade_unsupported_images(messages, model)
        .into_iter()
        .map(|message| match message {
            Message::User(_) => message,
            Message::ToolResult(mut tool_result) => {
                if let Some(normalized) = tool_call_id_map.get(&tool_result.tool_call_id) {
                    tool_result.tool_call_id = normalized.clone();
                }
                Message::ToolResult(tool_result)
            }
            Message::Assistant(mut assistant) => {
                let is_same_model = assistant.provider == model.provider
                    && assistant.api == model.api
                    && assistant.model == model.id;
                let source = assistant.clone();
                assistant.content = assistant
                    .content
                    .into_iter()
                    .filter_map(|block| match block {
                        AssistantContentBlock::Thinking(thinking) => {
                            transform_thinking(thinking, is_same_model)
                        }
                        AssistantContentBlock::Text(mut value) => {
                            if !is_same_model {
                                value.text_signature = None;
                            }
                            Some(AssistantContentBlock::Text(value))
                        }
                        AssistantContentBlock::ToolCall(mut tool_call) => {
                            if !is_same_model {
                                tool_call.thought_signature = None;
                                if let Some(normalize) = normalize_tool_call_id {
                                    let normalized = normalize(&tool_call.id, model, &source);
                                    if normalized != tool_call.id {
                                        tool_call_id_map
                                            .insert(tool_call.id.clone(), normalized.clone());
                                        tool_call.id = normalized;
                                    }
                                }
                            }
                            Some(AssistantContentBlock::ToolCall(tool_call))
                        }
                    })
                    .collect();
                Message::Assistant(assistant)
            }
        })
        .collect::<Vec<_>>();

    let mut result = Vec::with_capacity(transformed.len());
    let mut pending_tool_calls = Vec::new();
    let mut existing_tool_result_ids = HashSet::new();
    for message in transformed {
        match &message {
            Message::Assistant(assistant) => {
                insert_synthetic_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                if matches!(
                    assistant.stop_reason,
                    StopReason::Error | StopReason::Aborted
                ) {
                    continue;
                }
                pending_tool_calls = assistant
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContentBlock::ToolCall(tool_call) => Some(tool_call.clone()),
                        _ => None,
                    })
                    .collect();
                result.push(message);
            }
            Message::ToolResult(tool_result) => {
                existing_tool_result_ids.insert(tool_result.tool_call_id.clone());
                result.push(message);
            }
            Message::User(_) => {
                insert_synthetic_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                result.push(message);
            }
        }
    }
    insert_synthetic_tool_results(
        &mut result,
        &mut pending_tool_calls,
        &mut existing_tool_result_ids,
    );
    result
}

fn transform_thinking(
    thinking: ThinkingContent,
    is_same_model: bool,
) -> Option<AssistantContentBlock> {
    if thinking.redacted.unwrap_or(false) {
        return is_same_model.then_some(AssistantContentBlock::Thinking(thinking));
    }
    if thinking.thinking.trim().is_empty() {
        return None;
    }
    if is_same_model {
        Some(AssistantContentBlock::Thinking(thinking))
    } else {
        Some(AssistantContentBlock::Text(text(thinking.thinking)))
    }
}

/// Returns a cloned context whose messages have been normalized exactly once.
#[must_use]
pub fn transform_context(
    context: &crate::types::Context,
    model: &Model,
    normalize_tool_call_id: Option<&ToolCallIdNormalizer<'_>>,
) -> crate::types::Context {
    let mut transformed = context.clone();
    transformed.messages = transform_messages(&context.messages, model, normalize_tool_call_id);
    transformed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AssistantMessageRole, ThinkingContentType, ToolCallType, Usage, UserMessage,
        UserMessageRole,
    };

    fn model() -> Model {
        Model {
            id: "claude-sonnet-4.6".into(),
            api: "anthropic-messages".into(),
            provider: "github-copilot".into(),
            input: vec![ModelInput::Text, ModelInput::Image],
            ..Model::default()
        }
    }

    fn assistant(content: Vec<AssistantContentBlock>) -> Message {
        Message::Assistant(AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content,
            api: "openai-responses".into(),
            provider: "github-copilot".into(),
            model: "gpt-5".into(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: StopReason::ToolUse,
            error_message: None,
            timestamp: 0,
        })
    }

    #[test]
    fn converts_foreign_thinking_and_strips_signatures() {
        let result = transform_messages(
            &[assistant(vec![
                AssistantContentBlock::Thinking(ThinkingContent {
                    content_type: ThinkingContentType::Thinking,
                    thinking: "reason".into(),
                    thinking_signature: Some("foreign".into()),
                    redacted: Some(false),
                }),
                AssistantContentBlock::ToolCall(ToolCall {
                    content_type: ToolCallType::ToolCall,
                    id: "call|id".into(),
                    name: "tool".into(),
                    arguments: HashMap::new(),
                    thought_signature: Some("foreign".into()),
                }),
            ])],
            &model(),
            Some(&|id, _, _| id.replace('|', "_")),
        );
        let Message::Assistant(value) = &result[0] else {
            panic!("assistant")
        };
        assert!(
            matches!(&value.content[0], AssistantContentBlock::Text(text) if text.text == "reason")
        );
        assert!(
            matches!(&value.content[1], AssistantContentBlock::ToolCall(call) if call.id == "call_id" && call.thought_signature.is_none())
        );
        assert!(
            matches!(result.last(), Some(Message::ToolResult(result)) if result.tool_call_id == "call_id")
        );
    }

    #[test]
    fn inserts_missing_result_before_user() {
        let user = Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("next".into()),
            timestamp: 0,
        });
        let result = transform_messages(
            &[
                assistant(vec![AssistantContentBlock::ToolCall(ToolCall {
                    content_type: ToolCallType::ToolCall,
                    id: "call".into(),
                    name: "tool".into(),
                    arguments: HashMap::new(),
                    thought_signature: None,
                })]),
                user,
            ],
            &model(),
            None,
        );
        assert!(matches!(&result[1], Message::ToolResult(value) if value.is_error));
    }
}
