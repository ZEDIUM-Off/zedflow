//! Terminal text utilities ported from Pi's `tui/src/utils.ts`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnsiCode {
    pub code: String,
    pub length: usize,
}

/// Return an ANSI escape sequence beginning at `pos`, if complete.
pub fn extract_ansi_code(s: &str, pos: usize) -> Option<AnsiCode> {
    let b = s.as_bytes();
    if b.get(pos) != Some(&0x1b) {
        return None;
    }
    let kind = *b.get(pos + 1)?;
    let end = match kind {
        b'[' => b[pos + 2..]
            .iter()
            .position(|c| matches!(c, b'm' | b'G' | b'K' | b'H' | b'J'))
            .map(|i| pos + 2 + i + 1),
        b']' | b'_' => {
            let mut i = pos + 2;
            let mut found = None;
            while i < b.len() {
                if b[i] == 7 {
                    found = Some(i + 1);
                    break;
                }
                if b[i] == 0x1b && b.get(i + 1) == Some(&b'\\') {
                    found = Some(i + 2);
                    break;
                }
                i += 1;
            }
            found
        }
        _ => None,
    }?;
    Some(AnsiCode {
        code: s[pos..end].to_string(),
        length: end - pos,
    })
}

fn combining(c: char) -> bool {
    matches!(c as u32, 0x300..=0x36f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff | 0xfe20..=0xfe2f | 0xfe00..=0xfe0f | 0xe0100..=0xe01ef)
}
fn wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115f | 0x2329..=0x232a | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0x1f300..=0x1faff | 0x20000..=0x3fffd)
}
fn char_width(c: char) -> usize {
    if c.is_control() || combining(c) || matches!(c, '\u{200c}' | '\u{200d}') {
        0
    } else if wide(c) {
        2
    } else {
        1
    }
}
fn grapheme_width(segment: &str) -> usize {
    if segment == "\t" {
        return 3;
    }
    if emojis::get(segment).is_some() {
        return 2;
    }
    if segment
        .chars()
        .next()
        .is_some_and(|c| matches!(c as u32, 0x1f1e6..=0x1f1ff))
    {
        return 2;
    }
    segment.chars().map(char_width).sum()
}
fn graphemes(s: &str) -> Vec<&str> {
    let boundaries: Vec<_> = icu_segmenter::GraphemeClusterSegmenter::new()
        .segment_str(s)
        .collect();
    boundaries
        .windows(2)
        .map(|pair| &s[pair[0]..pair[1]])
        .collect()
}

/// Calculate terminal columns, ignoring supported ANSI/OSC/APC sequences.
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut i = 0;
    while i < s.len() {
        if let Some(a) = extract_ansi_code(s, i) {
            i += a.length;
            continue;
        }
        let end = s[i..].find('\x1b').map_or(s.len(), |offset| i + offset);
        width += graphemes(&s[i..end])
            .into_iter()
            .map(grapheme_width)
            .sum::<usize>();
        i = end;
        if i < s.len() && extract_ansi_code(s, i).is_none() {
            let c = s[i..].chars().next().unwrap();
            width += char_width(c);
            i += c.len_utf8();
        }
    }
    width
}

pub fn normalize_terminal_output(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '\u{e33}' => "\u{e4d}\u{e32}".chars().collect::<Vec<_>>(),
            '\u{eb3}' => "\u{ecd}\u{eb2}".chars().collect(),
            _ => vec![c],
        })
        .collect()
}
pub fn is_whitespace_char(c: &str) -> bool {
    c.chars().next().is_some_and(char::is_whitespace)
}
pub fn is_punctuation_char(c: &str) -> bool {
    c.chars()
        .next()
        .is_some_and(|c| "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`".contains(c))
}

fn take_width(s: &str, max: usize) -> (String, usize) {
    let mut out = String::new();
    let mut width = 0;
    let mut i = 0;
    while i < s.len() {
        if let Some(ansi) = extract_ansi_code(s, i) {
            out.push_str(&ansi.code);
            i += ansi.length;
            continue;
        }
        let end = s[i..].find('\x1b').map_or(s.len(), |offset| i + offset);
        let mut consumed = 0;
        for segment in graphemes(&s[i..end]) {
            let segment_width = grapheme_width(segment);
            if width + segment_width > max {
                return (out, width);
            }
            out.push_str(segment);
            width += segment_width;
            consumed += segment.len();
        }
        i += consumed;
        if consumed == 0 {
            let c = s[i..].chars().next().unwrap();
            let cw = char_width(c);
            if width + cw > max {
                break;
            }
            out.push(c);
            width += cw;
            i += c.len_utf8();
        }
    }
    (out, width)
}

pub fn truncate_to_width(s: &str, max_width: usize, ellipsis: &str, pad: bool) -> String {
    if max_width == 0 {
        return String::new();
    }
    let total = visible_width(s);
    if total <= max_width {
        return if pad {
            format!("{}{}", s, " ".repeat(max_width - total))
        } else {
            s.to_string()
        };
    }
    let ew = visible_width(ellipsis);
    if ew >= max_width {
        let (e, width) = take_width(ellipsis, max_width);
        if e.is_empty() {
            return e;
        }
        let result = format!("\x1b[0m{e}\x1b[0m");
        return if pad {
            format!("{result}{}", " ".repeat(max_width - width))
        } else {
            result
        };
    }
    let (prefix, pw) = take_width(s, max_width - ew);
    let result = format!("{}\x1b[0m{}\x1b[0m", prefix, ellipsis);
    if pad {
        format!("{}{}", result, " ".repeat(max_width - pw - ew))
    } else {
        result
    }
}

pub fn slice_by_column(s: &str, start: usize, length: usize, strict: bool) -> String {
    slice_with_width(s, start, length, strict).0
}
pub fn slice_with_width(s: &str, start: usize, length: usize, strict: bool) -> (String, usize) {
    if length == 0 {
        return (String::new(), 0);
    }
    let end_col = start.saturating_add(length);
    let mut out = String::new();
    let mut col = 0;
    let mut width = 0;
    let mut i = 0;
    while i < s.len() && col < end_col {
        if let Some(ansi) = extract_ansi_code(s, i) {
            if col >= start {
                out.push_str(&ansi.code);
            }
            i += ansi.length;
            continue;
        }
        let text_end = s[i..].find('\x1b').map_or(s.len(), |offset| i + offset);
        let mut consumed = 0;
        for segment in graphemes(&s[i..text_end]) {
            let cw = grapheme_width(segment);
            if col >= start && col < end_col && (!strict || col + cw <= end_col) {
                out.push_str(segment);
                width += cw;
            }
            col += cw;
            consumed += segment.len();
            if col >= end_col {
                break;
            }
        }
        i += consumed;
        if consumed == 0 {
            i += s[i..].chars().next().unwrap().len_utf8();
        }
    }
    (out, width)
}

pub fn apply_background_to_line(line: &str, width: usize, bg: impl Fn(&str) -> String) -> String {
    let padding = width.saturating_sub(visible_width(line));
    let mut s = line.to_string();
    s.push_str(&" ".repeat(padding));
    bg(&s)
}

fn update_active_sgr(text: &str, active: &mut String) {
    let mut i = 0;
    while i < text.len() {
        if let Some(ansi) = extract_ansi_code(text, i) {
            if ansi.code.ends_with('m') {
                if matches!(ansi.code.as_str(), "\x1b[m" | "\x1b[0m") {
                    active.clear();
                } else {
                    active.push_str(&ansi.code);
                }
            }
            i += ansi.length;
        } else {
            i += text[i..].chars().next().unwrap().len_utf8();
        }
    }
}

pub fn wrap_text_with_ansi(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    if width == 0 {
        return text.split('\n').map(str::to_string).collect();
    }
    text.split('\n')
        .flat_map(|line| {
            if visible_width(line) <= width {
                return vec![line.to_string()];
            }
            let mut lines = Vec::new();
            let mut rest = line;
            let mut active = String::new();
            while !rest.is_empty() {
                let (part, columns) = take_width(rest, width);
                if columns == 0 {
                    break;
                }
                let consumed = part.len();
                let mut output = format!("{active}{}", part.trim_end_matches(' '));
                update_active_sgr(&part, &mut active);
                if !active.is_empty() {
                    output.push_str("\x1b[0m");
                }
                lines.push(output);
                rest = &rest[consumed..];
            }
            lines
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn width_and_ansi() {
        assert_eq!(visible_width("a\u{301}"), 1);
        assert_eq!(visible_width("界"), 2);
        assert_eq!(visible_width("\x1b[31mred\x1b[0m"), 3);
    }
    #[test]
    fn truncation_and_slice() {
        assert_eq!(
            truncate_to_width("abcdef", 4, "...", false),
            "a\x1b[0m...\x1b[0m"
        );
        assert_eq!(slice_by_column("a界b", 1, 2, true), "界");
    }
}
