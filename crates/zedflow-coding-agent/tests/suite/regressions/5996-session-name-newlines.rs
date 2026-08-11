#[test]
fn status_text_stays_on_one_line() {
    assert_eq!(
        zedflow_coding_agent::footer::sanitize_status_text("hello\nworld\r\nagain"),
        "hello world again"
    );
}
