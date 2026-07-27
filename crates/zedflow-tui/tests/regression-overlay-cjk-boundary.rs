use zedflow_tui::utils::visible_width;
#[test]
fn overlay_width_handles_cjk_boundaries() {
    assert_eq!(visible_width("界"), 2);
    assert_eq!(visible_width("a界"), 3);
}
