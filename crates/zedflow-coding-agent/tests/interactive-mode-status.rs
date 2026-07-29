use zedflow_coding_agent::modes::interactive::InteractiveMode;
#[test]
fn status_is_available_after_update() {
    let mut mode = InteractiveMode::new();
    assert_eq!(mode.last_status(), None);
    mode.show_status("working");
    assert_eq!(mode.last_status(), Some("working"));
}
