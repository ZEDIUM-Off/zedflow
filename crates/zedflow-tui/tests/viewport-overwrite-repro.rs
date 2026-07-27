use zedflow_tui::utils::{normalize_terminal_output, visible_width};

#[test]
fn normalized_output_remains_displayable() {
    let output = normalize_terminal_output("hello\u{e33}");
    assert_eq!(visible_width(&output), 7);
}
