use zedflow_tui::utils::{visible_width, wrap_text_with_ansi};

#[test]
fn regional_indicator_and_emoji_width_is_stable() {
    assert_eq!(visible_width("🇺🇸"), 2);
    assert_eq!(visible_width("🇨"), 2);
    for sample in ["👍", "👍🏻", "✅", "⚡", "⚡️", "👨‍💻", "🏳️‍🌈"] {
        assert_eq!(visible_width(sample), 2, "{sample}");
    }
    let wrapped = wrap_text_with_ansi("      - 🇨", 9);
    assert_eq!(wrapped.len(), 2);
    assert_eq!(visible_width(&wrapped[0]), 7);
    assert_eq!(visible_width(&wrapped[1]), 2);
}
