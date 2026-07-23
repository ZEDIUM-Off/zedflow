use std::io;
pub fn copy_to_clipboard(text: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let command = ("pbcopy", &[][..]);
    #[cfg(target_os = "windows")]
    let command = ("clip", &[][..]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xclip", &["-selection", "clipboard"][..]);
    let mut child = std::process::Command::new(command.0)
        .args(command.1)
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    use std::io::Write;
    child.stdin.take().unwrap().write_all(text.as_bytes())?;
    child.wait().map(|_| ())
}
