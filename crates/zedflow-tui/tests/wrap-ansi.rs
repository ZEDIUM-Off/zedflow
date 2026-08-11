use zedflow_tui::utils::{visible_width, wrap_text_with_ansi};

#[test]
fn wraps_plain_and_cjk_text_at_terminal_columns() {
    let lines = wrap_text_with_ansi("hello world this is a test", 10);
    assert!(lines.len() > 1);
    assert!(lines.iter().all(|line| visible_width(line) <= 10));
    let lines = wrap_text_with_ansi(
        "This is an example 中文汉字测试段落内容中文汉字测试段落内容.",
        40,
    );
    assert_eq!(
        lines,
        [
            "This is an example 中文汉字测试段落内容",
            "中文汉字测试段落内容."
        ]
    );
    assert!(lines.iter().all(|line| visible_width(line) <= 40));
}

#[test]
fn preserves_ansi_sequences_and_osc_zero_width() {
    let lines = wrap_text_with_ansi("\x1b[31mred text that wraps\x1b[0m", 8);
    assert!(lines.iter().all(|line| visible_width(line) <= 8));
    assert!(lines[0].contains("\x1b[31m"));
    assert!(lines[0].ends_with("\x1b[0m"));
    assert!(lines[1].starts_with("\x1b[31m"));
    assert_eq!(visible_width("\x1b]133;A\x07hello\x1b]133;B\x07"), 5);
    assert_eq!(visible_width("🇨"), 2);
    assert_eq!(visible_width("🇨🇳"), 2);
}

#[test]
fn closes_and_reopens_osc8_links_at_word_boundary_wraps() {
    let open = "\x1b]8;;https://example.com\x1b\\";
    let close = "\x1b]8;;\x1b\\";
    let lines = wrap_text_with_ansi(&format!("{open}0123456789{close}"), 6);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with(open) && lines[0].ends_with(close));
    assert!(lines[1].starts_with(open) && lines[1].ends_with(close));
    assert!(lines.iter().all(|line| visible_width(line) <= 6));
}
