use zedflow_tui::{composite_line_at, utils::slice_by_column, visible_width};

#[test]
fn overlay_replaces_a_wide_grapheme_when_starting_inside_it() {
    let output = composite_line_at("abcd让EFGH", "│XX│", 5, 4, 20);
    let overlay = slice_by_column(&output, 5, 4, true);

    assert!(!output.contains('让'));
    assert_eq!(visible_width(&output), 20);
    assert!(overlay.contains("│XX│"));
}

#[test]
fn overlay_replaces_a_wide_grapheme_at_its_boundary() {
    let output = composite_line_at("abcd让EFGH", "│XX│", 4, 4, 20);
    assert!(!output.contains('让'));
    assert_eq!(visible_width(&output), 20);
    assert!(slice_by_column(&output, 4, 4, true).contains("│XX│"));
}
