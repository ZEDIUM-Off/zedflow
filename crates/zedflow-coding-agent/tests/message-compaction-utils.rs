use zedflow_ai::{
    Message, TextContent, TextContentType, ToolResultContentBlock, ToolResultMessage,
    ToolResultMessageRole, UserMessageContent,
};
use zedflow_coding_agent::core::{compaction::utils::serialize_conversation, messages::*};

#[test]
fn converts_summary_messages_with_pi_wrappers() {
    let message =
        create_compaction_summary_message("kept facts".to_string(), 42, "2024-01-02T03:04:05Z");

    let converted = convert_to_llm(&[message]);
    let Message::User(user) = &converted[0] else {
        panic!("summary must become a user message");
    };
    let UserMessageContent::Blocks(blocks) = &user.content else {
        panic!("summary must use structured content");
    };
    let zedflow_ai::UserContentBlock::Text(text) = &blocks[0] else {
        panic!("summary must contain text");
    };
    assert_eq!(
        text.text,
        format!("{COMPACTION_SUMMARY_PREFIX}kept facts{COMPACTION_SUMMARY_SUFFIX}")
    );
    assert_eq!(user.timestamp, 1_704_164_645_000);
}

#[test]
fn renders_bash_status_and_truncation() {
    let message = BashExecutionMessage {
        role: "bashExecution".to_string(),
        command: "cargo test".to_string(),
        output: "failed".to_string(),
        exit_code: Some(1),
        cancelled: false,
        truncated: true,
        full_output_path: Some("/tmp/full.log".to_string()),
        timestamp: 0,
        exclude_from_context: None,
    };

    assert_eq!(
        bash_execution_to_text(&message),
        "Ran `cargo test`\n```\nfailed\n```\n\nCommand exited with code 1\n\n[Output truncated. Full output: /tmp/full.log]"
    );
}

#[test]
fn truncates_only_tool_results_during_serialization() {
    let content = "x".repeat(2_001);
    let messages = [Message::ToolResult(ToolResultMessage {
        role: ToolResultMessageRole::ToolResult,
        tool_call_id: "call".to_string(),
        tool_name: "read".to_string(),
        content: vec![ToolResultContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: content,
            text_signature: None,
        })],
        details: None,
        is_error: false,
        timestamp: 0,
    })];

    let serialized = serialize_conversation(&messages);
    assert!(serialized.starts_with(&format!("[Tool result]: {}", "x".repeat(2_000))));
    assert!(serialized.ends_with("[... 1 more characters truncated]"));
}
