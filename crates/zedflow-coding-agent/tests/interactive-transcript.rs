use std::sync::Arc;

use serde_json::json;
use zedflow_agent::harness::{
    messages::{
        BranchSummaryMessage, CompactionSummaryMessage, CustomMessage, CustomMessageContent,
    },
    types::{CustomEntry, SessionTreeEntryBase},
};
use zedflow_coding_agent::{
    custom_entry::CustomEntryComponent,
    custom_message::CustomMessageComponent,
    diff::render_diff,
    modes_interactive_components_index::{
        assistant_message::{AssistantContent, StopReason, StreamingAssistantMessage},
        bash_execution::BashExecutionComponent,
        branch_summary_message::BranchSummaryMessageComponent,
        compaction_summary_message::CompactionSummaryMessageComponent,
    },
    skill_invocation_message::{ParsedSkillBlock, SkillInvocationMessageComponent},
    tool_execution::ToolExecutionComponent,
    user_message::UserMessageComponent,
    visual_truncate::truncate_to_visual_lines,
};
use zedflow_tui::{Component, Text};

fn rendered(component: &impl Component) -> String {
    component.render(80).join("\n")
}

#[test]
fn transcript_components_keep_real_incremental_content() {
    let mut assistant = StreamingAssistantMessage::default();
    assistant.update_snapshot(vec![
        AssistantContent::Thinking("draft".into()),
        AssistantContent::Text("first".into()),
    ]);
    assistant.update_snapshot(vec![
        AssistantContent::Thinking("final thought".into()),
        AssistantContent::Text("**final**".into()),
    ]);
    assistant.set_stop(StopReason::Length, None);
    let output = rendered(&assistant);
    assert!(
        output.contains("final thought")
            && output.contains("final")
            && output.contains("maximum output token limit")
    );
    assert!(!output.contains("draft") && !output.contains("first"));

    let user = rendered(&UserMessageComponent::new("1. actual markdown", 1));
    assert!(user.contains("1.") && user.contains("actual markdown"));

    let mut tool = ToolExecutionComponent::new("read", r#"{"path":"one"}"#);
    tool.update_args(r#"{"path":"two"}"#);
    tool.mark_execution_started();
    tool.set_args_complete();
    tool.update_result("file contents", false);
    let output = rendered(&tool);
    assert!(output.contains("two") && output.contains("file contents") && !output.contains("one"));

    let mut bash = BashExecutionComponent::new("printf hello", false);
    bash.append_output("hel");
    bash.append_output("lo\r\nworld");
    bash.set_complete(Some(0), false, None, None);
    assert_eq!(bash.get_output(), "hello\nworld");
    assert!(rendered(&bash).contains("hello"));
}

#[test]
fn summaries_custom_entries_diff_and_visual_tail_render_content() {
    let mut skill = SkillInvocationMessageComponent::new(ParsedSkillBlock {
        name: "demo".into(),
        content: "skill body".into(),
    });
    assert!(rendered(&skill).contains("demo"));
    skill.set_expanded(true);
    assert!(rendered(&skill).contains("skill body"));

    let mut compaction = CompactionSummaryMessageComponent::new(CompactionSummaryMessage {
        role: "compactionSummary".into(),
        summary: "compact body".into(),
        tokens_before: 12_345,
        timestamp: 0,
    });
    compaction.set_expanded(true);
    assert!(rendered(&compaction).contains("compact body"));
    let mut branch = BranchSummaryMessageComponent::new(BranchSummaryMessage {
        role: "branchSummary".into(),
        summary: "branch body".into(),
        from_id: "a".into(),
        timestamp: 0,
    });
    branch.set_expanded(true);
    assert!(rendered(&branch).contains("branch body"));

    let custom = CustomMessage {
        role: "custom".into(),
        custom_type: "notice".into(),
        content: CustomMessageContent::Text("custom body".into()),
        display: true,
        details: None,
        timestamp: 0,
    };
    assert!(rendered(&CustomMessageComponent::new(custom, None)).contains("custom body"));

    let entry = CustomEntry {
        base: SessionTreeEntryBase {
            id: "1".into(),
            parent_id: None,
            timestamp: "now".into(),
        },
        custom_type: "widget".into(),
        data: Some(json!({"ok": true})),
    };
    let renderer = Arc::new(|_: &CustomEntry, expanded| {
        Some(Box::new(Text::new(
            if expanded { "expanded entry" } else { "entry" },
            0,
            0,
        )) as Box<dyn Component>)
    });
    let mut entry = CustomEntryComponent::new(entry, renderer);
    entry.set_expanded(true);
    assert!(entry.has_content() && rendered(&entry).contains("expanded entry"));

    let diff = render_diff("-1 old\tvalue\n+1 new\tvalue");
    assert!(
        diff.contains("-1")
            && diff.contains("+1")
            && diff.contains("   value")
            && diff.contains("\x1b[7m")
    );
    let tail = truncate_to_visual_lines("one\ntwo\nthree", 2, 80, 0);
    assert_eq!(tail.skipped_count, 1);
    assert!(tail.visual_lines.join("\n").contains("three"));
}
