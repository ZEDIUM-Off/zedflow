pub fn is_apple_terminal_session() -> bool {
    std::env::var_os("TERM_PROGRAM").is_some_and(|v| v == "Apple_Terminal")
}
pub fn normalize_apple_terminal_input(data: &str, apple: bool, shift: bool) -> String {
    if apple && shift && data == "\x1b[3~" {
        "\x1b[3;2~".into()
    } else {
        data.into()
    }
}
