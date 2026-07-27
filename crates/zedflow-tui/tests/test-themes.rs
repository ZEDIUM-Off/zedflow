use zedflow_tui::utils::visible_width;

#[test]
fn theme_styling_codes_are_zero_width() {
    let styled = "\x1b[1mheading\x1b[0m";
    assert_eq!(visible_width(styled), 7);
}
