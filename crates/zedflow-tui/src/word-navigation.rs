//! Unicode-aware word navigation.

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036f}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05bd}'
            | '\u{05bf}'..='\u{05c7}'
            | '\u{0610}'..='\u{061a}'
            | '\u{064b}'..='\u{065f}'
            | '\u{0670}'
            | '\u{06d6}'..='\u{06ed}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{fe20}'..='\u{fe2f}'
            | '\u{e0100}'..='\u{e01ef}'
    )
}

// Intl.Segmenter keeps format characters such as joiners inside a word.
fn is_word_format(c: char) -> bool {
    matches!(c, '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}')
}

fn is_word_or_combining_mark(c: char) -> bool {
    is_word(c) || is_combining_mark(c) || is_word_format(c)
}

// Intl.Segmenter keeps CJK ideographs as separate word-like segments and
// treats the ASCII punctuation below as boundaries inside a word segment.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' |
        '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{2ffff}')
}
fn is_punctuation_boundary(c: char) -> bool {
    matches!(
        c,
        '(' | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | '<'
            | '>'
            | '.'
            | ','
            | ';'
            | ':'
            | '\''
            | '"'
            | '!'
            | '?'
            | '+'
            | '-'
            | '='
            | '*'
            | '/'
            | '\\'
            | '|'
            | '&'
            | '%'
            | '^'
            | '$'
            | '#'
            | '@'
            | '~'
            | '`'
    )
}
fn prev_char_start(text: &str, at: usize) -> Option<(usize, char)> {
    text.get(..at)
        .and_then(|prefix| prefix.char_indices().next_back())
}
pub fn find_word_backward(text: &str, cursor: usize) -> usize {
    let mut p = cursor.min(text.len());
    while let Some((start, c)) = prev_char_start(text, p) {
        if !c.is_whitespace() {
            break;
        }
        p = start;
    }
    let Some((start, c)) = prev_char_start(text, p) else {
        return p;
    };
    if is_cjk(c) {
        return start;
    }
    if is_word_or_combining_mark(c) {
        while let Some((start, c)) = prev_char_start(text, p) {
            if c.is_whitespace()
                || is_cjk(c)
                || is_punctuation_boundary(c)
                || !is_word_or_combining_mark(c)
            {
                break;
            }
            p = start;
        }
        return p;
    }
    while let Some((start, c)) = prev_char_start(text, p) {
        if c.is_whitespace() || is_word_or_combining_mark(c) || is_cjk(c) {
            break;
        }
        p = start;
    }
    p
}
pub fn find_word_forward(text: &str, cursor: usize) -> usize {
    let mut p = cursor.min(text.len());
    while let Some(c) = text.get(p..).and_then(|s| s.chars().next()) {
        if !c.is_whitespace() {
            break;
        }
        p += c.len_utf8();
    }
    let Some(c) = text.get(p..).and_then(|s| s.chars().next()) else {
        return p;
    };
    if is_cjk(c) {
        return p + c.len_utf8();
    }
    if is_word_or_combining_mark(c) {
        while let Some(c) = text.get(p..).and_then(|s| s.chars().next()) {
            if c.is_whitespace()
                || is_cjk(c)
                || is_punctuation_boundary(c)
                || !is_word_or_combining_mark(c)
            {
                break;
            }
            p += c.len_utf8();
        }
        return p;
    }
    while let Some(c) = text.get(p..).and_then(|s| s.chars().next()) {
        if c.is_whitespace() || is_word_or_combining_mark(c) || is_cjk(c) {
            break;
        }
        p += c.len_utf8();
    }
    p
}
