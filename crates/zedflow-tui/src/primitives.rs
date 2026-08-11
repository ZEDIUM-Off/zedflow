//! Terminal color response primitives.

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
    let max = 16f64.powi(s.len() as i32) - 1.0;
    if max <= 0.0 {
        return None;
    }
    let value = s.chars().try_fold(0f64, |value, c| {
        c.to_digit(16).map(|digit| value * 16.0 + f64::from(digit))
    })?;
    Some((value / max * 255.0).round() as u8)
}
fn parse_hex(s: &str) -> Option<RgbColor> {
    if !s.is_ascii() || !matches!(s.len(), 6 | 12) {
        return None;
    }
    if s.len() == 6 {
        Some(RgbColor {
            r: u8::from_str_radix(&s[0..2], 16).ok()?,
            g: u8::from_str_radix(&s[2..4], 16).ok()?,
            b: u8::from_str_radix(&s[4..6], 16).ok()?,
        })
    } else {
        Some(RgbColor {
            r: osc_channel(&s[0..4])?,
            g: osc_channel(&s[4..8])?,
            b: osc_channel(&s[8..12])?,
        })
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
