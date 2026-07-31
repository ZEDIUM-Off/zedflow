#[test]
fn tail_compaction_keeps_the_latest_lines() {
    let result = zedflow_coding_agent::truncate::truncate_tail(
        "one\ntwo\nthree\n",
        zedflow_coding_agent::truncate::TruncationOptions {
            max_lines: 1,
            max_bytes: 100,
        },
    );
    assert!(result.truncated);
    assert_eq!(result.content, "three");
}
