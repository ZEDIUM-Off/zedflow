use zedflow_coding_agent::footer::{format_tokens, sanitize_status_text};
#[test]
fn footer_compacts_tokens_and_status_whitespace() {
    assert_eq!(format_tokens(9_999), "10.0k");
    assert_eq!(format_tokens(10_000), "10k");
    assert_eq!(sanitize_status_text(" a\n\t b "), "a b");
}
