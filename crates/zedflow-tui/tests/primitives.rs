use zedflow_tui::*;

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
