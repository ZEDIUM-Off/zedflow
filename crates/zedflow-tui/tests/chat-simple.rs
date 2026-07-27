use zedflow_tui::utils::truncate_to_width;
#[test]
fn chat_lines_fit_the_requested_width() {
    assert_eq!(
        visible_width(&truncate_to_width("hello world", 5, "", false)),
        5
    );
}
fn visible_width(s: &str) -> usize {
    zedflow_tui::utils::visible_width(s)
}
