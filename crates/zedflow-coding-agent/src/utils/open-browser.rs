use std::io;

pub fn open_browser(target: &str) -> io::Result<()> {
    open_browser_with(target, |command| command.spawn().map(|_| ()))
}

pub fn open_browser_with(
    target: &str,
    open: impl FnOnce(&mut std::process::Command) -> io::Result<()>,
) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    open(command.arg(target))
}
