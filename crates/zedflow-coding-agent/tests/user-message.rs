#[test]
fn user_visible_footer_text_normalizes_control_whitespace() {
    assert_eq!(
        zedflow_coding_agent::footer::sanitize_status_text(" hello\t there "),
        "hello there"
    );
}
