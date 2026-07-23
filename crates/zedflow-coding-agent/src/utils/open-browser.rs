pub fn open_browser(target: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "rundll32";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(cmd)
            .args(["url.dll,FileProtocolHandler", target])
            .spawn()?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(cmd).arg(target).spawn()?;
    }
    Ok(())
}
