use zedflow_tui::terminal_colors::*;
#[test]
fn terminal_colors_parse_osc11() {
    assert!(is_osc11_background_color_response("\x1b]11;#102030\x07"));
    assert_eq!(
        parse_osc11_background_color("\x1b]11;#102030\x07")
            .unwrap()
            .r,
        16
    );
}
