//! Dependency-light, pure TUI primitives ported from Pi.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FuzzyMatch {
    pub matches: bool,
    pub score: f64,
}

pub fn fuzzy_match(query: &str, text: &str) -> FuzzyMatch {
    fn match_query(query: &str, text: &str) -> FuzzyMatch {
        if query.is_empty() {
            return FuzzyMatch {
                matches: true,
                score: 0.0,
            };
        }
        let q: Vec<char> = query.to_lowercase().chars().collect();
        let t: Vec<char> = text.to_lowercase().chars().collect();
        if q.len() > t.len() {
            return FuzzyMatch {
                matches: false,
                score: 0.0,
            };
        }
        let mut qi = 0;
        let mut score = 0.0;
        let mut last: Option<usize> = None;
        let mut consecutive = 0;
        for (i, c) in t.iter().enumerate() {
            if qi < q.len() && *c == q[qi] {
                let boundary =
                    i == 0 || matches!(t[i - 1], ' ' | '\t' | '\n' | '-' | '_' | '.' | '/' | ':');
                if last == i.checked_sub(1) {
                    consecutive += 1;
                    score -= consecutive as f64 * 5.0;
                } else {
                    consecutive = 0;
                    if let Some(previous) = last {
                        score += (i - previous - 1) as f64 * 2.0;
                    }
                }
                if boundary {
                    score -= 10.0;
                }
                score += i as f64 * 0.1;
                last = Some(i);
                qi += 1;
            }
        }
        if qi != q.len() {
            return FuzzyMatch {
                matches: false,
                score: 0.0,
            };
        }
        if q == t {
            score -= 100.0;
        }
        FuzzyMatch {
            matches: true,
            score,
        }
    }
    let primary = match_query(query, text);
    if primary.matches {
        return primary;
    }
    let chars: Vec<char> = query.to_lowercase().chars().collect();
    let split = chars.iter().position(|c| c.is_ascii_digit());
    let swapped = if let Some(i) = split {
        if i > 0 && chars[i..].iter().all(char::is_ascii_digit) {
            Some(chars[i..].iter().chain(&chars[..i]).collect::<String>())
        } else {
            None
        }
    } else {
        let i = chars.iter().position(|c| c.is_ascii_alphabetic());
        i.filter(|&i| {
            i > 0
                && chars[..i].iter().all(char::is_ascii_digit)
                && chars[i..].iter().all(|c| c.is_ascii_alphabetic())
        })
        .map(|i| chars[i..].iter().chain(&chars[..i]).collect())
    };
    swapped.map_or(primary, |q| {
        let m = match_query(&q, text);
        if m.matches {
            FuzzyMatch {
                matches: true,
                score: m.score + 5.0,
            }
        } else {
            primary
        }
    })
}

pub fn fuzzy_filter<T, F>(items: &[T], query: &str, get_text: F) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> &str,
{
    if query.trim().is_empty() {
        return items.to_vec();
    }
    let tokens: Vec<&str> = query
        .trim()
        .split(|c: char| c.is_whitespace() || c == '/')
        .filter(|s| !s.is_empty())
        .collect();
    let mut results: Vec<(T, f64)> = items
        .iter()
        .filter_map(|item| {
            let mut total = 0.0;
            for token in &tokens {
                let m = fuzzy_match(token, get_text(item));
                if !m.matches {
                    return None;
                }
                total += m.score;
            }
            Some((item.clone(), total))
        })
        .collect();
    results.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    results.into_iter().map(|(item, _)| item).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalColorScheme {
    Dark,
    Light,
}

fn osc_channel(s: &str) -> Option<u8> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let max = 16u32.pow(s.len() as u32).checked_sub(1)?;
    let value = u32::from_str_radix(s, 16).ok()?;
    Some(((value * 255 + max / 2) / max) as u8)
}
fn parse_hex(s: &str) -> Option<RgbColor> {
    if s.len() == 6 {
        Some(RgbColor {
            r: u8::from_str_radix(&s[0..2], 16).ok()?,
            g: u8::from_str_radix(&s[2..4], 16).ok()?,
            b: u8::from_str_radix(&s[4..6], 16).ok()?,
        })
    } else if s.len() == 12 {
        Some(RgbColor {
            r: osc_channel(&s[0..4])?,
            g: osc_channel(&s[4..8])?,
            b: osc_channel(&s[8..12])?,
        })
    } else {
        None
    }
}
pub fn is_osc11_background_color_response(data: &str) -> bool {
    let Some(value) = data.strip_prefix("\x1b]11;") else {
        return false;
    };
    let Some(value) = value
        .strip_suffix('\x07')
        .or_else(|| value.strip_suffix("\x1b\\"))
    else {
        return false;
    };
    !value.contains(['\x07', '\x1b'])
}
pub fn parse_osc11_background_color(data: &str) -> Option<RgbColor> {
    let value = data
        .strip_prefix("\x1b]11;")?
        .strip_suffix('\x07')
        .or_else(|| data.strip_prefix("\x1b]11;")?.strip_suffix("\x1b\\"))?
        .trim();
    if !is_osc11_background_color_response(data) {
        return None;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }
    let value = if value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rgba:"))
    {
        &value[5..]
    } else if value
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("rgb:"))
    {
        &value[4..]
    } else {
        value
    };
    let mut channels = value.split('/');
    Some(RgbColor {
        r: osc_channel(channels.next()?)?,
        g: osc_channel(channels.next()?)?,
        b: osc_channel(channels.next()?)?,
    })
}
pub fn parse_terminal_color_scheme_report(data: &str) -> Option<TerminalColorScheme> {
    match data {
        "\x1b[?997;1n" => Some(TerminalColorScheme::Dark),
        "\x1b[?997;2n" => Some(TerminalColorScheme::Light),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct KillRing {
    ring: Vec<String>,
}
impl KillRing {
    pub fn push(&mut self, text: &str, prepend: bool, accumulate: bool) {
        if text.is_empty() {
            return;
        }
        if accumulate && !self.ring.is_empty() {
            let last = self.ring.pop().unwrap();
            self.ring.push(if prepend {
                format!("{text}{last}")
            } else {
                format!("{last}{text}")
            });
        } else {
            self.ring.push(text.to_owned());
        }
    }
    pub fn peek(&self) -> Option<&str> {
        self.ring.last().map(String::as_str)
    }
    pub fn rotate(&mut self) {
        if self.ring.len() > 1 {
            let last = self.ring.pop().unwrap();
            self.ring.insert(0, last);
        }
    }
    pub fn len(&self) -> usize {
        self.ring.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UndoStack<S> {
    stack: Vec<S>,
}
impl<S: Clone> UndoStack<S> {
    pub fn push(&mut self, state: &S) {
        self.stack.push(state.clone());
    }
    pub fn pop(&mut self) -> Option<S> {
        self.stack.pop()
    }
    pub fn clear(&mut self) {
        self.stack.clear();
    }
    pub fn len(&self) -> usize {
        self.stack.len()
    }
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

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
