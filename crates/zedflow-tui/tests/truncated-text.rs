use zedflow_tui::utils::{truncate_to_width, visible_width};

#[test]
fn pads_truncated_content_to_requested_size() {
    let content = truncate_to_width("Hello", 38, "...", true);
    let line = format!(" {} ", content);
    assert_eq!(visible_width(&line), 40);
}

#[test]
fn truncates_only_the_first_line() {
    let first = "a very long first line";
    let content = truncate_to_width(first, 8, "...", false);
    assert_eq!(visible_width(&content), 8);
    assert!(!content.contains("second"));
}
