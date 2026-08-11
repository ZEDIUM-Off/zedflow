//! Rendering for numbered diffs, including compact intra-line highlighting.

use regex::Regex;
use std::sync::OnceLock;

fn parse_diff_line(line: &str) -> Option<(char, &str, &str)> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let captures = RE
        .get_or_init(|| Regex::new(r"^([+\-\s])(\s*\d*)\s(.*)$").unwrap())
        .captures(line)?;
    Some((
        captures[1].chars().next()?,
        captures.get(2)?.as_str(),
        captures.get(3)?.as_str(),
    ))
}

fn replace_tabs(text: &str) -> String {
    text.replace('\t', "   ")
}
fn inverse(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("\x1b[7m{text}\x1b[27m")
    }
}

fn intra_line(old: &str, new: &str) -> (String, String) {
    let old_chars: Vec<_> = old.chars().collect();
    let new_chars: Vec<_> = new.chars().collect();
    let prefix = old_chars
        .iter()
        .zip(&new_chars)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old_chars[prefix..]
        .iter()
        .rev()
        .zip(new_chars[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let render = |chars: &[char]| {
        let start: String = chars[..prefix].iter().collect();
        let changed: String = chars[prefix..chars.len().saturating_sub(suffix)]
            .iter()
            .collect();
        let end: String = chars[chars.len().saturating_sub(suffix)..].iter().collect();
        let leading: String = changed.chars().take_while(|c| c.is_whitespace()).collect();
        format!("{start}{leading}{}{end}", inverse(changed.trim_start()))
    };
    (render(&old_chars), render(&new_chars))
}

#[must_use]
pub fn render_diff(diff_text: &str) -> String {
    let lines: Vec<_> = diff_text.split('\n').collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((prefix, number, content)) = parse_diff_line(lines[i]) else {
            result.push(lines[i].to_owned());
            i += 1;
            continue;
        };
        if prefix == '-' {
            let mut removed = Vec::new();
            while i < lines.len() && parse_diff_line(lines[i]).is_some_and(|line| line.0 == '-') {
                let (_, n, c) = parse_diff_line(lines[i]).unwrap();
                removed.push((n, c));
                i += 1;
            }
            let mut added = Vec::new();
            while i < lines.len() && parse_diff_line(lines[i]).is_some_and(|line| line.0 == '+') {
                let (_, n, c) = parse_diff_line(lines[i]).unwrap();
                added.push((n, c));
                i += 1;
            }
            if removed.len() == 1 && added.len() == 1 {
                let (old, new) = intra_line(&replace_tabs(removed[0].1), &replace_tabs(added[0].1));
                result.push(format!("-{} {old}", removed[0].0));
                result.push(format!("+{} {new}", added[0].0));
            } else {
                result.extend(
                    removed
                        .into_iter()
                        .map(|(n, c)| format!("-{n} {}", replace_tabs(c))),
                );
                result.extend(
                    added
                        .into_iter()
                        .map(|(n, c)| format!("+{n} {}", replace_tabs(c))),
                );
            }
        } else {
            result.push(format!("{prefix}{number} {}", replace_tabs(content)));
            i += 1;
        }
    }
    result.join("\n")
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderDiffOptions {
    pub file_path: Option<String>,
}

#[must_use]
pub fn render_diff_with_options(diff_text: &str, _options: &RenderDiffOptions) -> String {
    render_diff(diff_text)
}
