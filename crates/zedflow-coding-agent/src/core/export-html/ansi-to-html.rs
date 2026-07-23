const COLORS: [&str; 16] = [
    "#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
    "#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
];

fn color256(index: usize) -> String {
    match index {
        0..=15 => COLORS[index].into(),
        16..=231 => {
            let n = index - 16;
            let component = |v: usize| if v == 0 { 0 } else { 55 + v * 40 };
            format!(
                "#{:02x}{:02x}{:02x}",
                component(n / 36),
                component((n % 36) / 6),
                component(n % 6)
            )
        }
        _ => {
            let gray = 8 + (index.saturating_sub(232)) * 10;
            format!("#{gray:02x}{gray:02x}{gray:02x}")
        }
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}
#[derive(Default, Clone)]
struct Style {
    fg: Option<String>,
    bg: Option<String>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
}
fn css(style: &Style) -> String {
    let mut out = Vec::new();
    if let Some(fg) = &style.fg {
        out.push(format!("color:{fg}"));
    }
    if let Some(bg) = &style.bg {
        out.push(format!("background-color:{bg}"));
    }
    if style.bold {
        out.push("font-weight:bold".into());
    }
    if style.dim {
        out.push("opacity:0.6".into());
    }
    if style.italic {
        out.push("font-style:italic".into());
    }
    if style.underline {
        out.push("text-decoration:underline".into());
    }
    out.join(";")
}
fn apply(params: &[usize], style: &mut Style) {
    let mut i = 0;
    while i < params.len() {
        let code = params[i];
        match code {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            22 => {
                style.bold = false;
                style.dim = false
            }
            23 => style.italic = false,
            24 => style.underline = false,
            30..=37 => style.fg = Some(COLORS[code - 30].into()),
            39 => style.fg = None,
            40..=47 => style.bg = Some(COLORS[code - 40].into()),
            49 => style.bg = None,
            90..=97 => style.fg = Some(COLORS[code - 82].into()),
            100..=107 => style.bg = Some(COLORS[code - 92].into()),
            38 | 48 if i + 2 < params.len() && params[i + 1] == 5 => {
                let value = color256(params[i + 2]);
                if code == 38 {
                    style.fg = Some(value);
                } else {
                    style.bg = Some(value);
                }
                i += 2
            }
            38 | 48 if i + 4 < params.len() && params[i + 1] == 2 => {
                let value = format!("rgb({},{},{})", params[i + 2], params[i + 3], params[i + 4]);
                if code == 38 {
                    style.fg = Some(value);
                } else {
                    style.bg = Some(value);
                }
                i += 4
            }
            _ => {}
        }
        i += 1;
    }
}

#[must_use]
pub fn ansi_to_html(text: &str) -> String {
    let mut out = String::new();
    let mut style = Style::default();
    let mut rest = text;
    while let Some(start) = rest.find("\x1b[") {
        out.push_str(&escape(&rest[..start]));
        let after = &rest[start + 2..];
        let Some(end) = after.find('m') else {
            out.push_str(&escape(&rest[start..]));
            return out;
        };
        let params = &after[..end];
        let values = if params.is_empty() {
            vec![0]
        } else {
            params.split(';').map(|v| v.parse().unwrap_or(0)).collect()
        };
        apply(&values, &mut style);
        rest = &after[end + 1..];
        if !css(&style).is_empty() {
            let next = rest.find("\x1b[").unwrap_or(rest.len());
            out.push_str(&format!(
                "<span style=\"{}\">{}</span>",
                css(&style),
                escape(&rest[..next])
            ));
            rest = &rest[next..];
        }
    }
    out.push_str(&escape(rest));
    out
}

#[must_use]
pub fn ansi_lines_to_html(lines: &[String]) -> String {
    lines
        .iter()
        .map(|line| {
            format!("<div class=\"ansi-line\">{}</div>", {
                let html = ansi_to_html(line);
                if html.is_empty() {
                    "&nbsp;".into()
                } else {
                    html
                }
            })
        })
        .collect()
}
