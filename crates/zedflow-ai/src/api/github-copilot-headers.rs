//! GitHub Copilot dynamic request headers ported from Pi.

use std::collections::HashMap;
use std::fmt;

/// Request initiator value expected by GitHub Copilot's `X-Initiator` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CopilotInitiator {
    /// The latest message is absent or user-authored.
    User,
    /// The latest message is assistant/tool-authored, such as a follow-up after tool use.
    Agent,
}

impl CopilotInitiator {
    /// Returns the Pi header string for this initiator.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for CopilotInitiator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Minimal Pi message shape consumed by the Copilot header helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// User-authored message.
    User {
        /// User content, either plain text or structured content parts.
        content: UserMessageContent,
    },
    /// Assistant-authored message.
    Assistant,
    /// Tool-result message.
    ToolResult {
        /// Tool-result content parts.
        content: Vec<MessageContent>,
    },
}

impl Message {
    /// Returns the Pi role string for this message.
    #[must_use]
    pub const fn role(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Assistant => "assistant",
            Self::ToolResult { .. } => "toolResult",
        }
    }
}

/// Content accepted by Pi user messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content parts.
    Parts(Vec<MessageContent>),
}

/// Structured message content part inspected by Copilot vision detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageContent {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Image content part.
    Image {
        /// Base64 encoded image data.
        data: String,
        /// Image MIME type, such as `image/png`.
        mime_type: String,
    },
}

impl MessageContent {
    /// Returns true when this content part is an image.
    #[must_use]
    pub const fn is_image(&self) -> bool {
        matches!(self, Self::Image { .. })
    }
}

/// Parameters for [`build_copilot_dynamic_headers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopilotDynamicHeadersParams<'a> {
    /// Conversation messages used to infer the Copilot initiator.
    pub messages: &'a [Message],
    /// Whether the request includes images.
    pub has_images: bool,
}

/// Infers the GitHub Copilot `X-Initiator` header value from Pi messages.
///
/// Pi returns `agent` when the latest message exists and is not user-authored;
/// otherwise it returns `user`.
#[must_use]
pub fn infer_copilot_initiator(messages: &[Message]) -> CopilotInitiator {
    match messages.last() {
        Some(last) if last.role() != "user" => CopilotInitiator::Agent,
        _ => CopilotInitiator::User,
    }
}

/// Returns true when Pi messages contain user/tool-result image input for Copilot.
#[must_use]
pub fn has_copilot_vision_input(messages: &[Message]) -> bool {
    messages.iter().any(|message| match message {
        Message::User {
            content: UserMessageContent::Parts(content),
        }
        | Message::ToolResult { content } => content.iter().any(MessageContent::is_image),
        Message::User {
            content: UserMessageContent::Text(_),
        }
        | Message::Assistant => false,
    })
}

/// Builds GitHub Copilot dynamic request headers.
#[must_use]
pub fn build_copilot_dynamic_headers(
    params: CopilotDynamicHeadersParams<'_>,
) -> HashMap<String, String> {
    let mut headers = HashMap::from([
        (
            "X-Initiator".to_string(),
            infer_copilot_initiator(params.messages).to_string(),
        ),
        (
            "Openai-Intent".to_string(),
            "conversation-edits".to_string(),
        ),
    ]);

    if params.has_images {
        headers.insert("Copilot-Vision-Request".to_string(), "true".to_string());
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_content() -> MessageContent {
        MessageContent::Image {
            data: "base64".to_string(),
            mime_type: "image/png".to_string(),
        }
    }

    #[test]
    fn infers_user_for_empty_or_latest_user_message() {
        assert_eq!(infer_copilot_initiator(&[]), CopilotInitiator::User);
        assert_eq!(
            infer_copilot_initiator(&[Message::User {
                content: UserMessageContent::Text("hello".to_string())
            }]),
            CopilotInitiator::User
        );
    }

    #[test]
    fn infers_agent_for_latest_non_user_message() {
        assert_eq!(
            infer_copilot_initiator(&[Message::ToolResult {
                content: Vec::new()
            }]),
            CopilotInitiator::Agent
        );
        assert_eq!(
            infer_copilot_initiator(&[Message::Assistant]),
            CopilotInitiator::Agent
        );
    }

    #[test]
    fn detects_user_or_tool_result_images_only() {
        assert!(!has_copilot_vision_input(&[Message::User {
            content: UserMessageContent::Text("plain".to_string())
        }]));
        assert!(has_copilot_vision_input(&[Message::User {
            content: UserMessageContent::Parts(vec![image_content()])
        }]));
        assert!(has_copilot_vision_input(&[Message::ToolResult {
            content: vec![image_content()]
        }]));
    }

    #[test]
    fn builds_dynamic_headers_with_optional_vision_header() {
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

        let headers = build_copilot_dynamic_headers(CopilotDynamicHeadersParams {
            messages: &messages,
            has_images: false,
        });
        assert!(!headers.contains_key("Copilot-Vision-Request"));
    }
}
