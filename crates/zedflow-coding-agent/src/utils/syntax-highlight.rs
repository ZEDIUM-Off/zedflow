use std::collections::HashMap;
pub type HighlightTheme = HashMap<String, fn(&str) -> String>;
pub fn render_highlighted_html(html: &str, _theme: &HighlightTheme) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        if c == '<' {
            in_tag = true
        }
        if !in_tag {
            out.push(c)
        }
        if c == '>' {
            in_tag = false
        }
    }
    out
}
pub fn highlight(code: &str, _language: Option<&str>, _theme: &HighlightTheme) -> String {
    code.to_owned()
}
pub fn supports_language(name: &str) -> bool {
    matches!(
        name,
        "rust" | "javascript" | "typescript" | "python" | "json" | "bash" | "shell"
    )
}
