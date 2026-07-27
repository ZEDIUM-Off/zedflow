use zedflow_tui::terminal::normalize_apple_terminal_input;

#[test]
fn normalizes_apple_shift_delete_without_touching_other_input() {
    assert_eq!(
        normalize_apple_terminal_input("\x1b[3~", true, true),
        "\x1b[3;2~"
    );
    assert_eq!(normalize_apple_terminal_input("\r", true, true), "\r");
    assert_eq!(
        normalize_apple_terminal_input("\x1b[3~", false, true),
        "\x1b[3~"
    );
}
