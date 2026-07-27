use zedflow_tui::{composite_line_at, visible_width};

#[test]
fn compositing_inserts_resets_between_styled_segments() {
    let base = format!("\x1b[3m{}\x1b[23m", "X".repeat(20));
    let line = composite_line_at(&base, "OVR", 5, 3, 20);

    assert_eq!(visible_width(&line), 20);
    assert!(line.matches("\x1b[0m\x1b]8;;\x07").count() >= 2);
}
