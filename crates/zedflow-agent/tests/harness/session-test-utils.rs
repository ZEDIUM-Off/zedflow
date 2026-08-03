use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use zedflow_agent::harness::types::{MessageEntry, SessionTreeEntry, SessionTreeEntryBase};
use zedflow_agent::types::{AgentMessage, Message};

pub fn user_message(text: &str) -> AgentMessage {
    AgentMessage::Custom(json!({
        "role": "user",
        "content": [{ "type": "text", "text": text }],
        "timestamp": 0
    }))
}

pub fn assistant_message(text: &str) -> AgentMessage {
    AgentMessage::Custom(json!({
        "role": "assistant",
        "content": [{ "type": "text", "text": text }],
        "api": "anthropic-messages",
        "provider": "anthropic",
        "model": "claude-sonnet-4-5",
        "usage": {
            "input": 0,
            "output": 0,
            "cacheRead": 0,
            "cacheWrite": 0,
            "totalTokens": 0,
            "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 }
        },
        "stopReason": "stop",
        "timestamp": 0
    }))
}

pub fn message_entry(id: &str, parent_id: Option<&str>, message: AgentMessage) -> SessionTreeEntry {
    SessionTreeEntry::Message(MessageEntry {
        base: entry_base(id, parent_id),
        message,
    })
}

pub fn entry_base(id: &str, parent_id: Option<&str>) -> SessionTreeEntryBase {
    SessionTreeEntryBase {
        id: id.to_string(),
        parent_id: parent_id.map(str::to_string),
        timestamp: "2026-01-01T00:00:00.000Z".to_string(),
    }
}

pub fn message_role(message: &AgentMessage) -> Option<&str> {
    match message {
        AgentMessage::Custom(value) => value.get("role").and_then(Value::as_str),
        AgentMessage::Llm(Message::User(_)) => Some("user"),
        AgentMessage::Llm(Message::Assistant(_)) => Some("assistant"),
        AgentMessage::Llm(Message::ToolResult(_)) => Some("toolResult"),
    }
}

pub fn message_roles(messages: &[AgentMessage]) -> Vec<&str> {
    messages.iter().filter_map(message_role).collect()
}

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "zedflow-agent-session-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn string(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_helpers_construct_expected_roles_and_content() {
        let user = user_message("hello");
        let assistant = assistant_message("world");

        assert_eq!(message_role(&user), Some("user"));
        assert_eq!(message_role(&assistant), Some("assistant"));
        assert_eq!(
            message_roles(&[user.clone(), assistant.clone()]),
            ["user", "assistant"]
        );
        assert_eq!(
            user,
            AgentMessage::Custom(json!({
                "role": "user",
                "content": [{ "type": "text", "text": "hello" }],
                "timestamp": 0
            }))
        );
        assert_eq!(assistant_role_and_text(&assistant), ("assistant", "world"));
    }

    #[test]
    fn temp_dir_is_removed_when_dropped() {
        let path = {
            let dir = TempDir::new();
            fs::write(dir.path().join("proof"), "present").expect("write temp file");
            assert!(dir.path().is_dir());
            assert_eq!(dir.string(), dir.path().to_string_lossy());
            dir.path().to_owned()
        };

        assert!(!path.exists());
    }

    fn assistant_role_and_text(message: &AgentMessage) -> (&str, &str) {
        let AgentMessage::Custom(value) = message else {
            panic!("expected custom assistant message");
        };
        (
            value["role"].as_str().expect("assistant role"),
            value["content"][0]["text"]
                .as_str()
                .expect("assistant text"),
        )
    }
}
