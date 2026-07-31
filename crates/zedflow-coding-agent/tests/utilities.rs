#[test]
fn utility_format_size_uses_readable_units() {
    assert_eq!(zedflow_coding_agent::truncate::format_size(1024), "1.0KB");
}
