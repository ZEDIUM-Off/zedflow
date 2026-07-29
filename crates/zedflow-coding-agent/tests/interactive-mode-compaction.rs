use zedflow_coding_agent::modes::interactive::InteractiveMode;
#[test]
fn status_updates_are_coalesced_to_latest_value() {
    let mut mode = InteractiveMode::new();
    mode.show_status("first");
    assert_eq!(mode.show_status("second"), "second");
    assert_eq!(mode.last_status(), Some("second"));
}
