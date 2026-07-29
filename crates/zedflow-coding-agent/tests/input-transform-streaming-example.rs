use zedflow_coding_agent::modes::interactive::InteractiveMode;
#[test]
fn interactive_input_is_trimmed_and_queued() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("  hello  ");
    assert_eq!(mode.get_user_input(), Some("hello".into()));
}
