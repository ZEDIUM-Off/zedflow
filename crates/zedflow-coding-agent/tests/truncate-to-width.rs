#[test]
fn line_truncation_respects_unicode_boundaries() {
    let (value, truncated) = zedflow_coding_agent::truncate::truncate_line("éclair", 1);
    assert!(truncated);
    assert_eq!(value, "é... [truncated]");
}
