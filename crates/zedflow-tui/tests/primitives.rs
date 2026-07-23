use zedflow_tui::*;

#[test]
fn parses_legacy_navigation_keys() {
    assert_eq!(parse_key("\x1b[A"), Some("up"));
    assert!(matches_key("\x1b[3~", "delete"));
    assert_eq!(parse_key("x"), None);
}

#[test]
fn fuzzy_matching_and_filtering() {
    assert!(fuzzy_match("abc", "aXbXc").matches);
    assert!(!fuzzy_match("abc", "cba").matches);
    assert!(fuzzy_match("codex52", "gpt-5.2-codex").matches);
    let items = ["clone", "cl"];
    assert_eq!(fuzzy_filter(&items, "cl", |s| s), vec!["cl", "clone"]);
}

#[test]
fn terminal_colors_parse_supported_responses() {
    assert_eq!(
        parse_osc11_background_color("\x1b]11;#123456\x07"),
        Some(RgbColor {
            r: 0x12,
            g: 0x34,
            b: 0x56
        })
    );
    assert_eq!(
        parse_osc11_background_color("\x1b]11;rgb:ffff/0000/8000\x1b\\"),
        Some(RgbColor {
            r: 255,
            g: 0,
            b: 128
        })
    );
    assert_eq!(
        parse_osc11_background_color("\x1b]11;RgB:0000/8000/ffff\x07"),
        Some(RgbColor {
            r: 0,
            g: 128,
            b: 255
        })
    );
    assert_eq!(
        parse_osc11_background_color("\x1b]11;rgb:ffffffff/80000000/01000000\x07"),
        Some(RgbColor {
            r: 255,
            g: 128,
            b: 1
        })
    );
    assert!(!is_osc11_background_color_response("x\x1b]11;#ffffff\x07"));
    assert!(!is_osc11_background_color_response("\x1b]11;#ffffff\x07x"));
    assert!(!is_osc11_background_color_response("\x1b]11;#ff\x1bff\x07"));
    assert_eq!(parse_osc11_background_color("\x1b]11;#ééé\x07"), None);
    assert_eq!(parse_osc11_background_color("\x1b]11;#12345\x07"), None);
    assert_eq!(
        parse_terminal_color_scheme_report("\x1b[?997;2n"),
        Some(TerminalColorScheme::Light)
    );
}

#[test]
fn kill_ring_accumulates_and_rotates() {
    let mut ring = KillRing::default();
    ring.push("one", false, false);
    ring.push("two", false, false);
    ring.push("!", false, true);
    assert_eq!(ring.peek(), Some("two!"));
    ring.rotate();
    assert_eq!(ring.peek(), Some("one"));
}

#[test]
fn undo_stack_clones_snapshots() {
    let mut stack = UndoStack::default();
    let mut state = vec![1, 2];
    stack.push(&state);
    state.push(3);
    assert_eq!(stack.pop(), Some(vec![1, 2]));
}

#[test]
fn word_navigation_skips_whitespace_and_runs() {
    let text = "hello, world";
    assert_eq!(find_word_forward(text, 0), 5);
    assert_eq!(find_word_forward(text, 6), text.len());
    assert_eq!(find_word_backward(text, text.len()), 7);
}

#[test]
fn word_navigation_keeps_combining_marks_with_their_word() {
    let text = "cafe\u{301} noir";
    assert_eq!(find_word_forward(text, 0), "cafe\u{301}".len());
    assert_eq!(find_word_backward(text, "cafe\u{301}".len()), 0);
}

#[test]
fn word_navigation_keeps_word_joiners_with_their_word() {
    let text = "a\u{200d}b a\u{fe0f}b";
    assert_eq!(find_word_forward(text, 0), "a\u{200d}b".len());
    assert_eq!(find_word_backward(text, "a\u{200d}b".len()), 0);
    let second = "a\u{200d}b ".len();
    assert_eq!(find_word_forward(text, second), text.len());
    assert_eq!(find_word_backward(text, text.len()), second);
}

#[test]
fn word_navigation_uses_unicode_word_boundaries() {
    let text = "你好世界 test";
    assert_eq!(find_word_forward(text, 0), "你".len());
    assert_eq!(find_word_forward(text, "你".len()), "你好".len());
    assert_eq!(
        find_word_backward(text, "你好世界".len()),
        "你好世界".len() - "界".len()
    );

    let text = "café déjà";
    assert_eq!(find_word_forward(text, 0), "café".len());
    assert_eq!(find_word_backward(text, text.len()), "café ".len());
}
