use zedflow_coding_agent::messages::{CustomMessageContent, convert_to_llm, create_custom_message};

#[test]
fn displayable_custom_message_reaches_the_model_context() {
    let message = create_custom_message(
        "test".into(),
        CustomMessageContent::Text("visible".into()),
        true,
        None,
        "1970-01-01T00:00:00Z",
    );
    assert_eq!(convert_to_llm(&[message]).len(), 1);
}
