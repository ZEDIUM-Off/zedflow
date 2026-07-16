//! GitHub Copilot dynamic request headers ported from Pi.

use std::collections::HashMap;

/// Minimal message shape retained for provider transports that normalize before header creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// User-authored message.
    User { content: UserMessageContent },
    /// Assistant-authored message.
    Assistant,
    /// Tool-result message.
    ToolResult { content: Vec<MessageContent> },
}

/// Content accepted by normalized Copilot user messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured content parts.
    Parts(Vec<MessageContent>),
}

/// Structured content inspected for vision input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageContent {
    /// Text content.
    Text { text: String },
    /// Image content.
    Image { data: String, mime_type: String },
}

/// Parameters for [`build_copilot_dynamic_headers`].
#[derive(Debug, Clone, Copy)]
pub struct CopilotDynamicHeadersParams<'a> {
    /// Normalized messages.
    pub messages: &'a [Message],
    /// Whether the request includes images.
    pub has_images: bool,
}

/// Infers `X-Initiator` from normalized messages.
#[must_use]
pub fn infer_copilot_initiator(messages: &[Message]) -> &'static str {
    match messages.last() {
        Some(Message::Assistant | Message::ToolResult { .. }) => "agent",
        Some(Message::User { .. }) | None => "user",
    }
}

/// Detects normalized user/tool-result image input.
#[must_use]
pub fn has_copilot_vision_input(messages: &[Message]) -> bool {
    messages.iter().any(|message| match message {
        Message::User {
            content: UserMessageContent::Parts(content),
        }
        | Message::ToolResult { content } => content
            .iter()
            .any(|part| matches!(part, MessageContent::Image { .. })),
        Message::User {
            content: UserMessageContent::Text(_),
        }
        | Message::Assistant => false,
    })
}

/// Builds dynamic headers from normalized messages.
#[must_use]
pub fn build_copilot_dynamic_headers(
    params: CopilotDynamicHeadersParams<'_>,
) -> HashMap<String, String> {
    dynamic_headers(infer_copilot_initiator(params.messages), params.has_images)
}

/// Infers `X-Initiator` directly from the canonical message contract.
#[must_use]
pub fn infer_copilot_initiator_from_context(messages: &[crate::types::Message]) -> &'static str {
    match messages.last() {
        Some(crate::types::Message::Assistant(_) | crate::types::Message::ToolResult(_)) => "agent",
        Some(crate::types::Message::User(_)) | None => "user",
    }
}

/// Detects canonical user/tool-result image input.
#[must_use]
pub fn has_copilot_vision_input_in_context(messages: &[crate::types::Message]) -> bool {
    use crate::types::{Message, ToolResultContentBlock, UserContentBlock, UserMessageContent};

    messages.iter().any(|message| match message {
        Message::User(message) => match &message.content {
            UserMessageContent::Blocks(content) => content
                .iter()
                .any(|part| matches!(part, UserContentBlock::Image(_))),
            UserMessageContent::Text(_) => false,
        },
        Message::ToolResult(message) => message
            .content
            .iter()
            .any(|part| matches!(part, ToolResultContentBlock::Image(_))),
        Message::Assistant(_) => false,
    })
}

/// Builds dynamic headers directly from canonical messages.
#[must_use]
pub fn build_copilot_dynamic_headers_for_context(
    messages: &[crate::types::Message],
) -> HashMap<String, String> {
    dynamic_headers(
        infer_copilot_initiator_from_context(messages),
        has_copilot_vision_input_in_context(messages),
    )
}

fn dynamic_headers(initiator: &str, has_images: bool) -> HashMap<String, String> {
    let mut headers = HashMap::from([
        ("X-Initiator".to_owned(), initiator.to_owned()),
        ("Openai-Intent".to_owned(), "conversation-edits".to_owned()),
    ]);
    if has_images {
        headers.insert("Copilot-Vision-Request".to_owned(), "true".to_owned());
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_exact_normalized_headers() {
        let messages = [Message::Assistant];
        let headers = build_copilot_dynamic_headers(CopilotDynamicHeadersParams {
            messages: &messages,
            has_images: true,
        });
        assert_eq!(
            headers.get("X-Initiator").map(String::as_str),
            Some("agent")
        );
        assert_eq!(
            headers.get("Openai-Intent").map(String::as_str),
            Some("conversation-edits")
        );
        assert_eq!(
            headers.get("Copilot-Vision-Request").map(String::as_str),
            Some("true")
        );
    }
}
