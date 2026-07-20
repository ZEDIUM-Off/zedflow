use zedflow_agent::harness::session::uuidv7;

#[test]
fn uses_rfc_9562_layout_and_preserves_monotonic_order() {
    let first = uuidv7();
    let second = uuidv7();

    assert_eq!(first.as_bytes().get(14), Some(&b'7'));
    assert!(matches!(
        first.as_bytes().get(19),
        Some(b'8' | b'9' | b'a' | b'b')
    ));
    assert!(
        first < second,
        "UUIDv7 values should sort by creation order"
    );
}
