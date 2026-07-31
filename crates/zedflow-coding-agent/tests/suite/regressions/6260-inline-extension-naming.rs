#[test]
fn extension_status_text_is_rendered_as_one_line() {
    assert_eq!(
        zedflow_coding_agent::footer::sanitize_status_text("inline\textension"),
        "inline extension"
    );
}
