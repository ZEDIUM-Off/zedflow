use serde_json::json;
use zedflow_agent::{
    harness::types::{
        MessageEntry, ModelChangeEntry, SessionTreeEntry, SessionTreeEntryBase,
        ThinkingLevelChangeEntry,
    },
    types::AgentMessage,
};
use zedflow_coding_agent::session_manager::build_session_context;

fn base(id: &str, parent_id: Option<&str>) -> SessionTreeEntryBase {
    SessionTreeEntryBase {
        id: id.into(),
        parent_id: parent_id.map(str::to_owned),
        timestamp: "2025-01-01T00:00:00.000Z".into(),
    }
}

#[test]
fn builds_context_from_messages_and_latest_settings() {
    let entries = vec![
        SessionTreeEntry::Message(MessageEntry {
            base: base("user", None),
            message: AgentMessage::Custom(json!({"role": "user", "content": "hello"})),
        }),
        SessionTreeEntry::ThinkingLevelChange(ThinkingLevelChangeEntry {
            base: base("thinking", Some("user")),
            thinking_level: "high".into(),
        }),
        SessionTreeEntry::ModelChange(ModelChangeEntry {
            base: base("model", Some("thinking")),
            provider: "openai".into(),
            model_id: "gpt-test".into(),
        }),
    ];

    let context = build_session_context(&entries);
    assert_eq!(context.messages.len(), 1);
    assert_eq!(context.thinking_level, "high");
    assert_eq!(context.model.unwrap().model_id, "gpt-test");
}
