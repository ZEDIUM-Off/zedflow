use zedflow_tui::utils::visible_width;
#[test]
fn image_line_detection_does_not_confuse_prefixes() {
    assert_eq!(visible_width("image"), 5);
    assert_eq!(visible_width("image2"), 6);
}
