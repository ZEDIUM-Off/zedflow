use zedflow_coding_agent::sdk::{CodexCacheProbeMessage, CodexCacheProbeSession};

#[test]
fn codex_cache_probe_is_append_only_and_runs_one_deterministic_tool_loop_per_turn() {
    let mut session = CodexCacheProbeSession::default();
    assert_eq!(
        CodexCacheProbeSession::LIVE_TRANSPORT_UNAVAILABLE,
        "live Codex transport requires configured credentials and is not available in the deterministic SDK probe"
    );

    let first = session.prompt_probe(1, "first").unwrap();
    let second = session.prompt_probe(2, "second").unwrap();

    assert_eq!(session.messages().len(), 8);
    assert!(matches!(
        &session.messages()[..4],
        [
            CodexCacheProbeMessage::User { turn: 1, marker },
            CodexCacheProbeMessage::Assistant(_),
            CodexCacheProbeMessage::ToolResult { turn: 1, marker: result_marker, .. },
            CodexCacheProbeMessage::Assistant(_),
        ] if marker == "first" && result_marker == "first"
    ));
    assert_eq!(
        first.tool_result,
        "deterministic_probe_result turn=1 marker=first fixed=OK"
    );
    assert_eq!(
        second.tool_result,
        "deterministic_probe_result turn=2 marker=second fixed=OK"
    );

    let cache_reads = [
        first.assistants[0].usage.cache_read,
        first.assistants[1].usage.cache_read,
        second.assistants[0].usage.cache_read,
        second.assistants[1].usage.cache_read,
    ];
    assert_eq!(cache_reads, [0, 1, 2, 3]);
    assert_eq!(first.assistants[0].subrequest, 1);
    assert_eq!(first.assistants[1].subrequest, 2);
}
