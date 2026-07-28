use std::sync::Arc;

use zedflow_tui::terminal_image::{TerminalCapabilities, set_capabilities};
use zedflow_tui::{Component, Markdown, visible_width};

fn plain(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .collect()
}

#[test]
fn renders_themed_markdown_before_wrapping_and_padding() {
    let mut markdown = Markdown::new("## hello `world`\n\n- alpha beta gamma").with_padding(1, 1);
    markdown.theme_mut().heading = Arc::new(|text| format!("\x1b[34m{text}\x1b[0m"));
    markdown.theme_mut().bold = Arc::new(|text| format!("\x1b[1m{text}\x1b[0m"));
    markdown.theme_mut().code = Arc::new(|text| format!("\x1b[31m{text}\x1b[0m"));

    let lines = markdown.render(14);

    assert_eq!(lines.first().unwrap(), "              ");
    assert_eq!(lines.last().unwrap(), "              ");
    assert!(lines.iter().all(|line| visible_width(line) == 14));
    assert!(
        lines.iter().any(|line| line.contains("\x1b[31mworld")),
        "{lines:?}"
    );
    assert!(plain(lines).iter().any(|line| line.contains("- alpha")));
}

#[test]
fn renders_lists_tables_quotes_and_strict_strikethrough() {
    let markdown = Markdown::new(
        "1. first\n   - nested\n2. second\n\n> quoted **bold**\n\n| A | B |\n| - | - |\n| one | two |\n\n~~gone~~ ~kept~",
    );
    let lines = plain(markdown.render(40));

    assert!(lines.iter().any(|line| line == "1. first"));
    assert!(lines.iter().any(|line| line == "    - nested"));
    assert!(lines.iter().any(|line| line.starts_with("│ quoted bold")));
    assert!(lines.iter().any(|line| line.starts_with("┌─")));
    assert!(lines.iter().any(|line| line.contains("gone ~kept~")));
}

#[test]
fn supports_source_marker_escape_options_and_cache_invalidation() {
    let mut markdown = Markdown::new("7. one\n7. two\n\nescaped \\*asterisk\\*");
    markdown.options_mut().preserve_ordered_list_markers = true;
    markdown.options_mut().preserve_backslash_escapes = true;

    let lines = plain(markdown.render(80));
    assert!(lines.iter().any(|line| line == "7. one"));
    assert!(lines.iter().any(|line| line == "7. two"));
    assert!(lines.iter().any(|line| line.contains("\\*asterisk\\*")));

    markdown.set_text("replacement");
    assert_eq!(plain(markdown.render(80)), ["replacement"]);
}

#[test]
fn emits_osc8_links_when_supported_and_falls_back_when_not() {
    let caps = |hyperlinks| TerminalCapabilities {
        images: None,
        true_color: true,
        hyperlinks,
    };
    let markdown = Markdown::new("[site](https://example.com)");

    set_capabilities(caps(true));
    let linked = markdown.render(80).join("\n");
    assert!(
        linked.contains("\x1b]8;;https://example.com\x1b\\site\x1b]8;;\x1b\\"),
        "{linked:?}"
    );

    markdown.invalidate();
    set_capabilities(caps(false));
    let fallback = markdown.render(80).join("\n");
    assert!(fallback.contains("site (https://example.com)"));
}
