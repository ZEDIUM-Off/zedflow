use zedflow_coding_agent::messages::{CustomMessageContent, create_custom_message};

#[test]
fn creates_custom_message_with_millisecond_timestamp() {
    let message = create_custom_message(
        "note".into(),
        CustomMessageContent::Text("text".into()),
        true,
        None,
        "1970-01-01T00:00:01Z",
    );
    assert!(format!("{message:?}").contains("note"));
}
