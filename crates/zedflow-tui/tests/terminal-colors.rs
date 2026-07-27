use zedflow_tui::terminal_colors::*;

#[test]
fn parses_strict_osc11_color_forms() {
    assert_eq!(
        parse_osc11_background_color("\x1b]11;rgb:0000/8000/ffff\x07"),
        Some(RgbColor {
            r: 0,
            g: 128,
            b: 255
        })
    );
    assert_eq!(
        parse_osc11_background_color("\x1b]11;#102030\x1b\\"),
        Some(RgbColor {
            r: 16,
            g: 32,
            b: 48
        })
    );
    assert!(is_osc11_background_color_response(
        "\x1b]11;not-a-color\x07"
    ));
    assert_eq!(parse_osc11_background_color("x\x1b]11;#ffffff\x07"), None);
    assert_eq!(parse_osc11_background_color("\x1b]10;#ffffff\x07"), None);
}

#[test]
fn parses_only_supported_color_scheme_reports() {
    assert_eq!(
        parse_terminal_color_scheme_report("\x1b[?997;1n"),
        Some(TerminalColorScheme::Dark)
    );
    assert_eq!(
        parse_terminal_color_scheme_report("\x1b[?997;2n"),
        Some(TerminalColorScheme::Light)
    );
    assert_eq!(parse_terminal_color_scheme_report("\x1b[?997;3n"), None);
}
