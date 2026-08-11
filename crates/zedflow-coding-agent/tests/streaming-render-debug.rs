#[path = "../src/modes/interactive/components/assistant-message.rs"]
mod assistant_message;

use assistant_message::StreamingAssistantMessage;

#[test]
fn streaming_snapshots_replace_partial_thinking_before_final_text() {
    let mut message = StreamingAssistantMessage::default();
    message.update_content("partial", "");
    message.update_content("complete thinking", "final answer");

    assert_eq!(message.thinking(), "complete thinking");
    assert_eq!(message.text(), "final answer");
}
