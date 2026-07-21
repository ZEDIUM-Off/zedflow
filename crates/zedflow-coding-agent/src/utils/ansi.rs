use regex::Regex;
use std::sync::OnceLock;

/// Removes ANSI OSC and CSI escape sequences from text.
#[must_use]
pub fn strip_ansi(value: &str) -> String {
    if !value.contains('\u{1b}') && !value.contains('\u{9b}') {
        return value.to_owned();
    }
    static ANSI: OnceLock<Regex> = OnceLock::new();
    ANSI.get_or_init(|| {
        Regex::new(r"(?s:\x1B\].*?(?:\x07|\x1B\\|\x{9C}))|[\x1B\x{9B}][\[\]()#;?]*(?:\d{1,4}(?:[;:]\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]").expect("valid ANSI regex")
    })
    .replace_all(value, "")
    .into_owned()
}
