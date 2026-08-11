use zedflow_coding_agent::session_selector::renamed_session_name;

#[test]
fn rename_submits_text_at_the_picker_cursor() {
    assert_eq!(renamed_session_name("X", "Old"), "XOld");
}
