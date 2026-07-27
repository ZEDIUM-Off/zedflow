use zedflow_tui::utils::visible_width;
#[test]
fn regional_indicator_width_is_stable() {
    assert_eq!(visible_width("🇺🇸"), 2);
}
