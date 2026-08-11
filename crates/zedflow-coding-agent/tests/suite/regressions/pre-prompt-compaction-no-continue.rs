#[test]
fn truncation_reports_that_content_was_compacted() {
    let result = zedflow_coding_agent::truncate::truncate_head(
        "one\ntwo\n",
        zedflow_coding_agent::truncate::TruncationOptions {
            max_lines: 1,
            max_bytes: 100,
        },
    );
    assert!(result.truncated);
    assert_eq!(result.content, "one");
}
