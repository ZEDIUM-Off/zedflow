use zedflow_tui::utils::{slice_by_column, visible_width};

#[test]
fn autocomplete_candidates_preserve_terminal_columns() {
    assert_eq!(visible_width("hello"), 5);
    assert_eq!(slice_by_column("hello", 1, 3, false), "ell");
}
