use zedflow_tui::utils::{apply_background_to_line, visible_width};

#[test]
fn background_padding_does_not_change_visible_width() {
    let line = apply_background_to_line("\x1b[3mXXXXX\x1b[23m", 20, |s| {
        format!("\x1b[44m{s}\x1b[0m")
    });
    assert_eq!(visible_width(&line), 20);
    assert!(line.starts_with("\x1b[44m"));
}
