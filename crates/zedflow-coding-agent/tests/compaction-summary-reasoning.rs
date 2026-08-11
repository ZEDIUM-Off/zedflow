use zedflow_coding_agent::messages::{CustomMessageContent, convert_to_llm, create_custom_message};

#[test]
fn displayable_custom_messages_are_forwarded_to_the_model() {
    let message = create_custom_message(
        "local".into(),
        CustomMessageContent::Text("private".into()),
        true,
        None,
        "1970-01-01T00:00:00Z",
    );
    assert_eq!(convert_to_llm(&[message]).len(), 1);
}
