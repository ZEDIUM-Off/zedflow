use serde_json::json;
use zedflow_agent::{
    harness::types::{AgentHarnessEvent, AgentHarnessOwnEvent, QueueUpdateEvent},
    types::{AgentEvent, AgentMessage},
};
use zedflow_coding_agent::modes::interactive::InteractiveMode;
use zedflow_tui::{Component, ProcessTerminal};

#[test]
fn session_lifecycle_updates_one_stateful_interactive_tree_in_pi_order() {
    let mut mode = InteractiveMode::with_terminal(ProcessTerminal::new());
    let user = AgentMessage::Custom(json!({"role":"user","content":"hello"}));
    let first = AgentMessage::Custom(json!({
        "role":"assistant",
        "content":[{"type":"text","text":"draft"}]
    }));
    let final_message = AgentMessage::Custom(json!({
        "role":"assistant",
        "content":[{"type":"text","text":"answer"}]
    }));

    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageStart {
        message: user.clone(),
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageStart {
        message: first,
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageUpdate {
        message: final_message.clone(),
        assistant_message_event: serde_json::from_value(json!({
            "type":"text_delta",
            "contentIndex":0,
            "delta":"answer",
            "partial": {
                "role":"assistant", "content":[], "api":"openai-completions",
                "provider":"openai", "model":"test", "usage": {
                    "input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                    "cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}
                }, "stopReason":"stop", "timestamp":0
            }
        }))
        .unwrap(),
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageEnd {
        message: final_message,
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionStart {
        tool_call_id: "call-1".into(),
        tool_name: "read".into(),
        args: json!({"path":"a.txt"}),
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionUpdate {
        tool_call_id: "call-1".into(),
        tool_name: "read".into(),
        args: json!({"path":"a.txt"}),
        partial_result: json!({"content":[{"type":"text","text":"partial"}]}),
    }));
    mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionEnd {
        tool_call_id: "call-1".into(),
        tool_name: "read".into(),
        result: json!({"content":[{"type":"text","text":"complete"}]}),
        is_error: false,
    }));
    mode.apply_session_event(AgentHarnessEvent::Harness(
        AgentHarnessOwnEvent::QueueUpdate(QueueUpdateEvent {
            steer: vec![user],
            follow_up: vec![],
            next_turn: vec![],
        }),
    ));

    let transcript = mode.rendered_transcript(80).join("\n");
    assert!(transcript.contains("hello") && transcript.contains("answer"));
    assert!(transcript.contains("a.txt") && transcript.contains("complete"));
    assert!(!transcript.contains("draft") && !transcript.contains("partial"));

    mode.run().unwrap();
    let tree = mode.tui_mut().root.render(80).join("\n");
    mode.stop().unwrap();
    assert!(tree.find("hello").unwrap() < tree.find("1 queued message(s)").unwrap());
    assert!(tree.find("1 queued message(s)").unwrap() < tree.find("no-model").unwrap());
}

#[test]
fn live_tree_uses_custom_editor_submission_and_rendering() {
    let mut mode = InteractiveMode::with_terminal(ProcessTerminal::new());
    mode.run().unwrap();
    mode.tui_mut().dispatch_input("hello");
    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.get_user_input().as_deref(), Some("hello"));
    assert!(
        mode.tui_mut()
            .root
            .render(80)
            .iter()
            .any(|line| line.contains('─'))
    );
    mode.stop().unwrap();
}

#[test]
fn settings_selector_enter_and_escape_restore_editor_dispatch() {
    let mut mode = InteractiveMode::with_terminal(ProcessTerminal::new());
    mode.run().unwrap();

    mode.tui_mut().dispatch_input("/settings");
    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.tui_mut().overlay_count(), 1);

    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.tui_mut().overlay_count(), 0);
    assert_eq!(mode.last_status(), Some("Theme saved to settings"));

    mode.tui_mut().dispatch_input("/settings");
    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.tui_mut().overlay_count(), 1);
    mode.tui_mut().dispatch_input("\x1b");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.tui_mut().overlay_count(), 0);
    mode.tui_mut().dispatch_input("editor restored");
    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert_eq!(mode.get_user_input().as_deref(), Some("editor restored"));
    mode.stop().unwrap();
}

#[test]
fn custom_editor_keeps_ctrl_d_on_the_interactive_exit_queue() {
    let mut mode = InteractiveMode::with_terminal(ProcessTerminal::new());
    mode.run().unwrap();
    mode.tui_mut().dispatch_input("\x04");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    assert!(mode.exit_requested());
    mode.stop().unwrap();
}
