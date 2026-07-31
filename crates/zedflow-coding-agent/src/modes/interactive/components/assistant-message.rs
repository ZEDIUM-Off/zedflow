//! Pi-compatible streaming assistant-message state.

/// The visible content accumulated while an assistant message streams.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamingAssistantMessage {
    thinking: String,
    text: String,
}

impl StreamingAssistantMessage {
    /// Replaces the streamed snapshot, rather than appending it twice.
    pub fn update_content(&mut self, thinking: impl Into<String>, text: impl Into<String>) {
        self.thinking = thinking.into();
        self.text = text.into();
    }

    #[must_use]
    pub fn thinking(&self) -> &str {
        &self.thinking
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}
