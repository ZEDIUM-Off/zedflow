use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_agent::harness::compaction::branch_summarization::{
    GenerateBranchSummaryOptions, collect_entries_for_branch_summary, generate_branch_summary,
    prepare_branch_entries,
};
use zedflow_agent::harness::compaction::compaction::{
    DEFAULT_COMPACTION_SETTINGS, calculate_context_tokens, compact, estimate_context_tokens,
    estimate_tokens, find_cut_point, find_turn_start_index, generate_summary,
    get_last_assistant_usage, prepare_compaction, should_compact,
};
use zedflow_agent::harness::compaction::utils::serialize_conversation;
use zedflow_agent::harness::session::{
    InMemorySessionStorage, InMemorySessionStorageOptions, Session,
};
use zedflow_agent::harness::types::{
    BranchSummaryEntry, CompactionEntry, CompactionPreparation, CompactionSettings,
    CustomMessageContent, CustomMessageEntry, FileOperations, MessageEntry, ModelChangeEntry,
    SessionMetadata, SessionTreeEntry, SessionTreeEntryBase, ThinkingLevelChangeEntry,
};
use zedflow_agent::types::{AgentMessage, ThinkingLevel};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, Context, CreateProviderOptions, DoneStopReason, ErrorStopReason, Message,
    Model, ProviderApi, ProviderAuth, ProviderStreams, SimpleStreamOptions, StopReason,
    TextContent, TextContentType, ThinkingLevel as AiThinkingLevel, ToolCall, ToolCallType,
    ToolResultContentBlock, ToolResultMessage, ToolResultMessageRole, Usage, UsageCost,
    UserMessage, UserMessageContent, UserMessageRole, create_models, create_provider,
};

fn usage(input: u64, output: u64, cache_read: u64, cache_write: u64) -> Usage {
    Usage {
        input,
        output,
        cache_read,
        cache_write,
        cache_write_1h: None,
        reasoning: None,
        total_tokens: input + output + cache_read + cache_write,
        cost: UsageCost::default(),
    }
}

fn user_text(text: impl Into<String>) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Text(text.into()),
        timestamp: 1,
    }))
}

fn assistant_text(text: impl Into<String>, usage: Usage) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.into(),
            text_signature: None,
        })],
        api: "fake-api".into(),
        provider: "fake-provider".into(),
        model: "fake-model".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage,
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 1,
    }
}

fn assistant_message(text: impl Into<String>, usage: Usage) -> AgentMessage {
    AgentMessage::Llm(Message::Assistant(assistant_text(text, usage)))
}

fn assistant_tool_call(name: &str, path: &str) -> AgentMessage {
    AgentMessage::Llm(Message::Assistant(AssistantMessage {
        content: vec![AssistantContentBlock::ToolCall(ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".into(),
            name: name.into(),
            arguments: HashMap::from([("path".into(), json!(path))]),
            thought_signature: None,
        })],
        ..assistant_text("", usage(100, 50, 0, 0))
    }))
}

fn tool_result_text(text: impl Into<String>) -> Message {
    Message::ToolResult(ToolResultMessage::<Value> {
        role: ToolResultMessageRole::ToolResult,
        tool_call_id: "tc1".into(),
        tool_name: "read".into(),
        content: vec![ToolResultContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.into(),
            text_signature: None,
        })],
        details: None,
        is_error: false,
        timestamp: 1,
    })
}

fn base(id: &str, parent_id: Option<&str>) -> SessionTreeEntryBase {
    SessionTreeEntryBase {
        id: id.into(),
        parent_id: parent_id.map(str::to_string),
        timestamp: "2026-01-01T00:00:00.000Z".into(),
    }
}

fn message_entry(id: &str, parent_id: Option<&str>, message: AgentMessage) -> SessionTreeEntry {
    SessionTreeEntry::Message(MessageEntry {
        base: base(id, parent_id),
        message,
    })
}

fn compaction_entry(
    id: &str,
    parent_id: Option<&str>,
    summary: &str,
    first_kept_entry_id: &str,
    details: Option<Value>,
) -> SessionTreeEntry {
    SessionTreeEntry::Compaction(CompactionEntry {
        base: base(id, parent_id),
        summary: summary.into(),
        first_kept_entry_id: first_kept_entry_id.into(),
        tokens_before: 1234,
        details,
        from_hook: None,
    })
}

fn branch_summary_entry(
    id: &str,
    parent_id: Option<&str>,
    from_id: &str,
    summary: &str,
    details: Option<Value>,
) -> SessionTreeEntry {
    SessionTreeEntry::BranchSummary(BranchSummaryEntry {
        base: base(id, parent_id),
        from_id: from_id.into(),
        summary: summary.into(),
        details,
        from_hook: None,
    })
}

fn custom_message_entry(id: &str, parent_id: Option<&str>, content: &str) -> SessionTreeEntry {
    SessionTreeEntry::CustomMessage(CustomMessageEntry {
        base: base(id, parent_id),
        custom_type: "note".into(),
        content: CustomMessageContent::Text(content.into()),
        details: None,
        display: true,
    })
}

#[test]
fn calculates_thresholds_cut_points_and_context_estimates() {
    assert_eq!(calculate_context_tokens(&usage(1000, 500, 200, 100)), 1800);
    assert_eq!(calculate_context_tokens(&usage(0, 0, 0, 0)), 0);

    let settings = CompactionSettings {
        enabled: true,
        reserve_tokens: 10_000,
        keep_recent_tokens: 20_000,
    };
    assert!(should_compact(95_000, 100_000, settings));
    assert!(!should_compact(89_000, 100_000, settings));
    assert!(!should_compact(
        95_000,
        100_000,
        CompactionSettings {
            enabled: false,
            ..settings
        }
    ));

    let thinking = SessionTreeEntry::ThinkingLevelChange(ThinkingLevelChangeEntry {
        base: base("thinking", None),
        thinking_level: "high".into(),
    });
    let model_change = SessionTreeEntry::ModelChange(ModelChangeEntry {
        base: base("model", Some("thinking")),
        provider: "openai".into(),
        model_id: "gpt-4".into(),
    });
    assert_eq!(
        find_cut_point(&[thinking.clone(), model_change.clone()], 0, 2, 1).first_kept_entry_index,
        0
    );
    assert_eq!(
        find_turn_start_index(&[thinking.clone(), model_change], 1, 0),
        None
    );

    let entries = vec![
        message_entry("u1", None, user_text("hello")),
        message_entry(
            "a1",
            Some("u1"),
            assistant_message("answer", usage(10, 5, 3, 2)),
        ),
        message_entry("u2", Some("a1"), user_text("tail")),
    ];
    let estimate = estimate_context_tokens(
        &entries
            .iter()
            .filter_map(message_from_entry)
            .collect::<Vec<_>>(),
    );
    assert_eq!(estimate.usage_tokens, 20);
    assert_eq!(estimate.last_usage_index, Some(1));
    assert!(estimate.trailing_tokens > 0);
    assert_eq!(get_last_assistant_usage(&entries), Some(usage(10, 5, 3, 2)));
}

#[test]
fn estimates_supported_message_roles_and_serializes_tool_results() {
    assert!(estimate_tokens(&user_text("plain user")) > 0);
    assert!(estimate_tokens(&assistant_tool_call("read", "src/lib.rs")) > 0);
    assert!(
        estimate_tokens(&AgentMessage::Custom(json!({
            "role": "custom",
            "content": "custom text"
        }))) > 0
    );
    assert!(
        estimate_tokens(&AgentMessage::Custom(json!({
            "role": "toolResult",
            "content": [{ "type": "image", "mimeType": "image/png", "data": "abc" }]
        }))) > 1000
    );
    assert!(
        estimate_tokens(&AgentMessage::Custom(json!({
            "role": "bashExecution",
            "command": "cargo test",
            "output": "ok"
        }))) > 0
    );
    assert_eq!(
        estimate_tokens(&AgentMessage::Custom(json!({ "role": "unknown" }))),
        0
    );

    let result = serialize_conversation(&[tool_result_text("x".repeat(5000))]);
    assert!(result.contains("[Tool result]:"));
    assert!(result.contains("[... 3000 more characters truncated]"));
}

#[test]
fn prepares_compaction_with_previous_summary_split_turn_and_file_details() {
    let entries = vec![
        message_entry("u1", None, user_text("user msg 1")),
        message_entry("a1", Some("u1"), assistant_tool_call("write", "written.rs")),
        compaction_entry(
            "c1",
            Some("a1"),
            "First summary",
            "u1",
            Some(json!({ "readFiles": ["old-read.rs"], "modifiedFiles": ["old-edit.rs"] })),
        ),
        message_entry("u2", Some("c1"), user_text("large turn")),
        message_entry(
            "a2",
            Some("u2"),
            assistant_message("large assistant", usage(100, 50, 0, 0)),
        ),
    ];

    let preparation = prepare_compaction(
        &entries,
        CompactionSettings {
            enabled: true,
            reserve_tokens: 100,
            keep_recent_tokens: 1,
        },
    )
    .expect("prepare")
    .expect("compaction preparation");

    assert_eq!(
        preparation.previous_summary.as_deref(),
        Some("First summary")
    );
    assert!(preparation.is_split_turn);
    assert_eq!(preparation.turn_prefix_messages.len(), 1);
    assert!(preparation.file_ops.read.contains("old-read.rs"));
    assert!(preparation.file_ops.edited.contains("old-edit.rs"));
    assert!(preparation.file_ops.written.contains("written.rs"));

    let branch_and_custom = vec![
        branch_summary_entry("bs", None, "branch", "branch summary", None),
        custom_message_entry("cm", Some("bs"), "custom content"),
        message_entry("u3", Some("cm"), user_text("keep")),
        message_entry(
            "a3",
            Some("u3"),
            assistant_message("assistant", usage(100, 50, 0, 0)),
        ),
    ];
    let prepared_custom = prepare_compaction(
        &branch_and_custom,
        CompactionSettings {
            enabled: true,
            reserve_tokens: 100,
            keep_recent_tokens: 1,
        },
    )
    .expect("prepare")
    .expect("custom preparation");
    assert_eq!(prepared_custom.messages_to_summarize.len(), 2);

    assert!(
        prepare_compaction(&[], DEFAULT_COMPACTION_SETTINGS)
            .expect("prepare")
            .is_none()
    );
    assert!(
        prepare_compaction(
            &[compaction_entry("c", None, "done", "u", None)],
            DEFAULT_COMPACTION_SETTINGS
        )
        .expect("prepare")
        .is_none()
    );
}

#[test]
fn prepares_branch_entries_and_collects_abandoned_branch() {
    let root = message_entry("u1", None, user_text("root"));
    let target = message_entry(
        "target",
        Some("u1"),
        assistant_message("kept branch", usage(10, 5, 0, 0)),
    );
    let old_user = message_entry("old-user", Some("u1"), user_text("old branch"));
    let old_assistant = message_entry(
        "old-assistant",
        Some("old-user"),
        assistant_tool_call("read", "read-only.rs"),
    );
    let old_tool = message_entry(
        "old-tool",
        Some("old-assistant"),
        AgentMessage::Llm(tool_result_text("tool output")),
    );

    let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
        entries: Some(vec![
            root.clone(),
            target.clone(),
            old_user.clone(),
            old_assistant.clone(),
            old_tool.clone(),
        ]),
        metadata: Some(SessionMetadata {
            id: "session".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
        }),
    }))
    .expect("storage");
    let session = Session::new(storage);

    let collected = block_on(collect_entries_for_branch_summary(
        &session,
        Some("old-tool"),
        "target",
    ))
    .expect("collect");
    assert_eq!(collected.common_ancestor_id.as_deref(), Some("u1"));
    assert_eq!(
        entry_ids(&collected.entries),
        vec!["old-user", "old-assistant", "old-tool"]
    );

    let details = json!({ "readFiles": ["prior-read.rs"], "modifiedFiles": ["prior-edit.rs"] });
    let prepared = prepare_branch_entries(
        &[
            branch_summary_entry("bs", None, "old", "branch", Some(details)),
            old_user,
            old_assistant,
            old_tool,
        ],
        100_000,
    );
    assert_eq!(prepared.messages.len(), 3);
    assert!(prepared.file_ops.read.contains("read-only.rs"));
    assert!(prepared.file_ops.read.contains("prior-read.rs"));
    assert!(prepared.file_ops.edited.contains("prior-edit.rs"));
}

#[tokio::test]
async fn generates_summaries_with_fake_models_and_parity_assertions() {
    let (models, model, seen) = fake_models(
        true,
        128_000,
        vec![
            fake_response("## Goal\nTest summary", StopReason::Stop, None),
            fake_response("## Goal\nCompact history", StopReason::Stop, None),
            fake_response(
                "## Original Request\nPrefix summary",
                StopReason::Stop,
                None,
            ),
            fake_response("## Goal\nBranch summary", StopReason::Stop, None),
        ],
    );

    let summary = generate_summary(
        &[user_text("Summarize this.")],
        &models,
        &model,
        2000,
        Some("focus"),
        Some("old summary"),
        Some(ThinkingLevel::Medium),
    )
    .await
    .expect("summary");
    assert!(summary.contains("Test summary"));
    assert_eq!(seen.reasonings(), vec![Some(AiThinkingLevel::Medium)]);
    assert!(seen.prompts()[0].contains("<previous-summary>\nold summary\n</previous-summary>"));
    assert!(seen.prompts()[0].contains("Additional focus: focus"));

    let preparation = CompactionPreparation {
        first_kept_entry_id: "entry-keep".into(),
        messages_to_summarize: vec![user_text("history")],
        turn_prefix_messages: vec![user_text("prefix")],
        is_split_turn: true,
        tokens_before: 600_000,
        previous_summary: None,
        file_ops: FileOperations {
            read: HashSet::from(["read.rs".into()]),
            written: HashSet::from(["write.rs".into()]),
            edited: HashSet::new(),
        },
        settings: CompactionSettings {
            enabled: true,
            reserve_tokens: 500_000,
            keep_recent_tokens: 20_000,
        },
    };
    let compacted = compact(
        &preparation,
        &models,
        &model,
        None,
        Some(ThinkingLevel::High),
    )
    .await
    .expect("compact");
    assert!(compacted.summary.contains("Turn Context (split turn)"));
    assert!(
        compacted
            .summary
            .contains("<read-files>\nread.rs\n</read-files>")
    );
    assert!(
        compacted
            .summary
            .contains("<modified-files>\nwrite.rs\n</modified-files>")
    );
    assert_eq!(
        seen.max_tokens(),
        vec![Some(1600), Some(128_000), Some(128_000)]
    );

    let branch = generate_branch_summary(
        &[message_entry("u", None, user_text("branch work"))],
        GenerateBranchSummaryOptions {
            models: &models,
            model: &model,
            signal: None,
            custom_instructions: Some("branch focus"),
            replace_instructions: false,
            reserve_tokens: Some(16_384),
        },
    )
    .await
    .expect("branch summary");
    assert!(
        branch
            .summary
            .starts_with("The user explored a different conversation branch")
    );
    assert!(branch.summary.contains("Branch summary"));
    assert!(seen.prompts()[3].contains("Additional focus: branch focus"));

    // No live provider calls: the local fake provider handled deterministic queued responses only.
    assert_eq!(
        models
            .get_provider("fake-provider")
            .expect("provider")
            .get_models()
            .len(),
        1
    );
}

#[tokio::test]
async fn returns_summary_errors_without_live_calls() {
    let (models, model, _) = fake_models(
        false,
        4096,
        vec![
            fake_response("", StopReason::Error, Some("boom")),
            fake_response("", StopReason::Aborted, Some("stopped")),
        ],
    );

    let error = generate_summary(
        &[user_text("Summarize this.")],
        &models,
        &model,
        2000,
        None,
        None,
        None,
    )
    .await
    .expect_err("error response should fail");
    assert_eq!(
        error.code,
        zedflow_agent::harness::types::CompactionErrorCode::SummarizationFailed
    );
    assert_eq!(error.message, "Summarization failed: boom");

    let aborted = generate_summary(
        &[user_text("Summarize this.")],
        &models,
        &model,
        2000,
        None,
        None,
        None,
    )
    .await
    .expect_err("aborted response should fail");
    assert_eq!(
        aborted.code,
        zedflow_agent::harness::types::CompactionErrorCode::Aborted
    );
    assert_eq!(aborted.message, "stopped");
}

#[test]
#[ignore = "live provider behavior is intentionally excluded from AT4; deterministic fake providers cover compaction parity without network/model calls"]
fn live_provider_compaction_smoke_is_not_run() {}

fn message_from_entry(entry: &SessionTreeEntry) -> Option<AgentMessage> {
    match entry {
        SessionTreeEntry::Message(entry) => Some(entry.message.clone()),
        _ => None,
    }
}

fn entry_ids(entries: &[SessionTreeEntry]) -> Vec<&str> {
    entries
        .iter()
        .map(|entry| match entry {
            SessionTreeEntry::Message(entry) => entry.base.id.as_str(),
            SessionTreeEntry::BranchSummary(entry) => entry.base.id.as_str(),
            SessionTreeEntry::Compaction(entry) => entry.base.id.as_str(),
            SessionTreeEntry::CustomMessage(entry) => entry.base.id.as_str(),
            SessionTreeEntry::ThinkingLevelChange(entry) => entry.base.id.as_str(),
            SessionTreeEntry::ModelChange(entry) => entry.base.id.as_str(),
            SessionTreeEntry::ActiveToolsChange(entry) => entry.base.id.as_str(),
            SessionTreeEntry::Custom(entry) => entry.base.id.as_str(),
            SessionTreeEntry::Label(entry) => entry.base.id.as_str(),
            SessionTreeEntry::SessionInfo(entry) => entry.base.id.as_str(),
            SessionTreeEntry::Leaf(entry) => entry.base.id.as_str(),
        })
        .collect()
}

#[derive(Clone, Default)]
struct SeenCalls {
    prompts: Arc<Mutex<Vec<String>>>,
    reasonings: Arc<Mutex<Vec<Option<AiThinkingLevel>>>>,
    max_tokens: Arc<Mutex<Vec<Option<u32>>>>,
}

impl SeenCalls {
    fn prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("prompts").clone()
    }

    fn reasonings(&self) -> Vec<Option<AiThinkingLevel>> {
        self.reasonings.lock().expect("reasonings").clone()
    }

    fn max_tokens(&self) -> Vec<Option<u32>> {
        self.max_tokens.lock().expect("max tokens").clone()
    }
}

fn fake_models(
    reasoning: bool,
    max_tokens: u64,
    responses: Vec<AssistantMessage>,
) -> (zedflow_ai::Models, Model, SeenCalls) {
    let model = Model {
        provider: "fake-provider".into(),
        id: "fake-model".into(),
        name: "Fake Model".into(),
        api: "fake-api".into(),
        base_url: "http://localhost:0".into(),
        reasoning,
        context_window: 200_000,
        max_tokens,
        ..Model::default()
    };
    let seen = SeenCalls::default();
    let pending = Arc::new(Mutex::new(VecDeque::from(responses)));
    let stream_seen = seen.clone();
    let stream_pending = Arc::clone(&pending);

    let provider = create_provider(CreateProviderOptions {
        id: "fake-provider".into(),
        name: Some("Fake".into()),
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![model.clone()],
        refresh_models: None,
        api: ProviderApi::Single(ProviderStreams {
            stream: Arc::new(move |model, context, _options| {
                fake_stream(model, context, None, &stream_pending, &stream_seen)
            }),
            stream_simple: {
                let seen = seen.clone();
                let pending = Arc::clone(&pending);
                Arc::new(
                    move |model, context, options: Option<&SimpleStreamOptions>| {
                        fake_stream(model, context, options, &pending, &seen)
                    },
                )
            },
        }),
    });

    let mut models = create_models();
    models.set_provider(provider);
    (models, model, seen)
}

fn fake_stream(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
    pending: &Arc<Mutex<VecDeque<AssistantMessage>>>,
    seen: &SeenCalls,
) -> AssistantMessageEventStream {
    seen.prompts
        .lock()
        .expect("prompts")
        .push(prompt_text(context));
    seen.reasonings
        .lock()
        .expect("reasonings")
        .push(options.and_then(|options| options.reasoning));
    seen.max_tokens
        .lock()
        .expect("max tokens")
        .push(options.and_then(|options| options.stream.max_tokens));

    let mut message = pending
        .lock()
        .expect("pending responses")
        .pop_front()
        .unwrap_or_else(|| {
            fake_response("", StopReason::Error, Some("No more fake responses queued"))
        });
    message.provider.clone_from(&model.provider);
    message.api.clone_from(&model.api);
    message.model.clone_from(&model.id);

    let stream = AssistantMessageEventStream::new();
    match message.stop_reason {
        StopReason::Aborted => stream.push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            error: message,
        }),
        StopReason::Error => stream.push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Error,
            error: message,
        }),
        StopReason::Length => stream.push(AssistantMessageEvent::Done {
            reason: DoneStopReason::Length,
            message,
        }),
        StopReason::ToolUse => stream.push(AssistantMessageEvent::Done {
            reason: DoneStopReason::ToolUse,
            message,
        }),
        StopReason::Stop => stream.push(AssistantMessageEvent::Done {
            reason: DoneStopReason::Stop,
            message,
        }),
    }
    stream
}

fn prompt_text(context: &Context) -> String {
    let Some(Message::User(message)) = context.messages.first() else {
        return String::new();
    };
    match &message.content {
        UserMessageContent::Text(text) => text.clone(),
        UserMessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                zedflow_ai::UserContentBlock::Text(block) => Some(block.text.as_str()),
                zedflow_ai::UserContentBlock::Image(_) => None,
            })
            .collect(),
    }
}

fn fake_response(
    text: &str,
    stop_reason: StopReason,
    error_message: Option<&str>,
) -> AssistantMessage {
    AssistantMessage {
        stop_reason,
        error_message: error_message.map(str::to_string),
        ..assistant_text(text, Usage::default())
    }
}
