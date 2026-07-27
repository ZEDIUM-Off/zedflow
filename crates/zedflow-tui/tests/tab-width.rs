use zedflow_tui::utils::visible_width;
#[test]
fn tab_width_matches_pi() {
    assert_eq!(visible_width("a\tb"), 5);
}
