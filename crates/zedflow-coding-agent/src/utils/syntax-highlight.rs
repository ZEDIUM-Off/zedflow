use std::collections::HashMap;
pub type HighlightTheme = HashMap<String, fn(&str) -> String>;
pub fn render_highlighted_html(html: &str, theme: &HighlightTheme) -> String {
    let mut out = String::new();
    let mut text = String::new();
    let mut scopes: Vec<Option<String>> = vec![];
    let flush = |text: &mut String, scopes: &Vec<Option<String>>, out: &mut String| {
        if text.is_empty() {
            return;
        }
        let formatter = scopes
            .iter()
            .rev()
            .flatten()
            .find_map(|s| theme.get(s))
            .or_else(|| theme.get("default"));
        if let Some(f) = formatter {
            out.push_str(&f(text));
        } else {
            out.push_str(text);
        }
        text.clear();
    };
    let mut i = 0;
    while i < html.len() {
        if html[i..].starts_with("<span")
            && html
                .as_bytes()
                .get(i + 5)
                .is_some_and(|c| matches!(c, b'>' | b' ' | b'\t' | b'\n' | b'\r'))
        {
            if let Some(end) = html[i + 5..].find('>') {
                flush(&mut text, &scopes, &mut out);
                let tag = &html[i..i + 6 + end];
                let scope = tag
                    .split("class=")
                    .nth(1)
                    .and_then(|x| x.trim_start_matches(['"', '\'']).split(['"', '\'']).next())
                    .and_then(|x| {
                        x.split_whitespace()
                            .find_map(|c| c.strip_prefix("hljs-"))
                            .map(str::to_owned)
                    });
                scopes.push(scope);
                i += 6 + end;
                continue;
            }
        }
        if html[i..].starts_with("</span>") {
            flush(&mut text, &scopes, &mut out);
            scopes.pop();
            i += 7;
            continue;
        }
        if let Some(end) = html[i..].find(';') {
            if html.as_bytes()[i] == b'&' && end <= 16 {
                let entity = &html[i + 1..i + end];
                if let Some(decoded) = crate::utils::html::decode_html_entity(entity) {
                    text.push_str(&decoded);
                    i += end + 1;
                    continue;
                }
            }
        }
        let c = html[i..].chars().next().unwrap();
        text.push(c);
        i += c.len_utf8();
    }
    flush(&mut text, &scopes, &mut out);
    out
}
pub fn supports_language(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "rust"
            | "javascript"
            | "typescript"
            | "python"
            | "json"
            | "html"
            | "css"
            | "bash"
            | "shell"
            | "markdown"
            | "diff"
    )
}
pub fn highlight(code: &str, _language: Option<&str>, theme: &HighlightTheme) -> String {
    render_highlighted_html(code, theme)
}
