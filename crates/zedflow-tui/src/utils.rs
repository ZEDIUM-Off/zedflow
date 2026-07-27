/// Width policy shared by rendering and navigation: control characters are zero,
/// combining marks are zero, and wide East Asian/emoji scalars count as two.
pub fn visible_width(text: &str) -> usize {
    text.chars()
        .map(|c| {
            if c.is_control() || is_combining(c) {
                0
            } else if is_wide(c) {
                2
            } else {
                1
            }
        })
        .sum()
}
fn is_combining(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
        || ('\u{1ab0}'..='\u{1aff}').contains(&c)
        || ('\u{1dc0}'..='\u{1dff}').contains(&c)
        || ('\u{20d0}'..='\u{20ff}').contains(&c)
        || ('\u{fe20}'..='\u{fe2f}').contains(&c)
}
fn is_wide(c: char) -> bool {
    ('\u{1100}'..='\u{115f}').contains(&c)
        || ('\u{2329}'..='\u{232a}').contains(&c)
        || ('\u{2e80}'..='\u{a4cf}').contains(&c)
        || ('\u{ac00}'..='\u{d7a3}').contains(&c)
        || ('\u{f900}'..='\u{faff}').contains(&c)
        || ('\u{1f300}'..='\u{1faff}').contains(&c)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composite_width_policy() {
        assert_eq!(visible_width("a\u{301}"), 1);
        assert_eq!(visible_width("界"), 2);
    }
}

pub fn truncate_to_width(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let n = visible_width(&c.to_string());
        if used + n > width {
            break;
        }
        out.push(c);
        used += n;
    }
    out
}
pub fn slice_by_column(text: &str, start: usize, end: usize) -> String {
    let mut out = String::new();
    let mut col = 0;
    for c in text.chars() {
        let n = visible_width(&c.to_string());
        if col + n > start && col < end {
            out.push(c)
        }
        col += n;
    }
    out
}
pub fn wrap_text_with_ansi(text: &str, width: usize) -> Vec<String> {
    text.lines()
        .flat_map(|l| {
            if width == 0 {
                return vec![l.to_string()];
            };
            let cs: Vec<char> = l.chars().collect();
            cs.chunks(width)
                .map(|x| x.iter().collect())
                .collect::<Vec<_>>()
        })
        .collect()
}
