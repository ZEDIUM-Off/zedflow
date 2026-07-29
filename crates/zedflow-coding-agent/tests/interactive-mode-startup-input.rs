use zedflow_coding_agent::modes::interactive::InteractiveMode;
#[test]
fn blank_startup_input_is_ignored() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input(" ");
    mode.queue_user_input("start");
    assert_eq!(mode.pending_user_input_count(), 1);
    assert_eq!(mode.get_user_input(), Some("start".into()));
}
