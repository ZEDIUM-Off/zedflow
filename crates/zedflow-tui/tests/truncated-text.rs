use zedflow_tui::{Component, TruncatedText, visible_width};

#[test]
fn pads_and_truncates_the_first_line_like_pi() {
    let text = TruncatedText::new(
        "This is a very long first line that needs truncation\nSecond line",
        1,
        2,
    );
    let lines = text.render(25);
    assert_eq!(lines.len(), 5);
    assert!(lines.iter().all(|line| visible_width(line) == 25));
    assert!(lines[2].contains("..."));
    assert!(!lines[2].contains("Second"));
}

#[test]
fn preserves_ansi_and_resets_before_the_ellipsis() {
    let text = TruncatedText::new(
        "\x1b[31mThis is a very long red text that will be truncated\x1b[0m",
        1,
        0,
    );
    let lines = text.render(20);
    assert_eq!(visible_width(&lines[0]), 20);
    assert!(lines[0].contains("\x1b[0m..."), "{:?}", lines[0]);
}

#[test]
fn empty_text_still_renders_one_padded_content_line() {
    let lines = TruncatedText::new("", 1, 0).render(30);
    assert_eq!(lines, [" ".repeat(30)]);
}
