use zedflow_tui::utils::{truncate_to_width, visible_width};

#[test]
fn truncation_stays_within_width_for_unicode_and_ansi() {
    let value = truncate_to_width(&"🙂界".repeat(1_000), 40, "…", false);
    assert!(visible_width(&value) <= 40);
    let styled = truncate_to_width("\x1b[31mhello hello hello\x1b[0m", 10, "…", false);
    assert!(styled.contains("\x1b[31m"));
    assert_eq!(visible_width(&styled), 10);
}

#[test]
fn malformed_escape_does_not_consume_plain_text_forever() {
    let value = truncate_to_width("abc\x1bnot-ansi 🙂", 6, "…", false);
    assert!(visible_width(&value) <= 6);
}
