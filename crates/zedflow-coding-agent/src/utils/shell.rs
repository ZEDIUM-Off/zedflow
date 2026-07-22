use std::env;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellConfig {
    pub shell: String,
    pub args: Vec<String>,
    pub command_transport_stdin: bool,
}
fn legacy_wsl(path: &str) -> bool {
    let p = path.replace('/', "\\").to_ascii_lowercase();
    p.ends_with("\\windows\\system32\\bash.exe") || p.ends_with("\\windows\\sysnative\\bash.exe")
}
fn bash(path: String) -> ShellConfig {
    let stdin = legacy_wsl(&path);
    ShellConfig {
        command_transport_stdin: stdin,
        shell: path,
        args: vec![if stdin { "-s" } else { "-c" }.into()],
    }
}
pub fn get_shell_config(custom: Option<&Path>) -> Result<ShellConfig, String> {
    if let Some(path) = custom {
        if !path.exists() {
            return Err(format!("Custom shell path not found: {}", path.display()));
        }
        return Ok(bash(path.to_string_lossy().into_owned()));
    }
    if cfg!(windows) {
        for key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = env::var_os(key) {
                let path = Path::new(&root).join("Git/bin/bash.exe");
                if path.exists() {
                    return Ok(bash(path.to_string_lossy().into_owned()));
                }
            }
        }
    }
    if Path::new("/bin/bash").exists() {
        return Ok(bash("/bin/bash".into()));
    }
    if env::var_os("PATH").is_some() {
        return Ok(bash("bash".into()));
    }
    Ok(ShellConfig {
        shell: "sh".into(),
        args: vec!["-c".into()],
        command_transport_stdin: false,
    })
}
pub fn get_shell_env(bin_dir: &Path) -> Vec<(String, String)> {
    let mut envs: Vec<_> = env::vars().collect();
    let key = envs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "PATH".into());
    if let Some((_, path)) = envs
        .iter_mut()
        .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
    {
        let bin = bin_dir.to_string_lossy();
        if !path
            .split(if cfg!(windows) { ';' } else { ':' })
            .any(|p| p == bin)
        {
            *path = format!(
                "{bin}{sep}{path}",
                sep = if cfg!(windows) { ';' } else { ':' }
            );
        }
    } else {
        envs.push((key, bin_dir.to_string_lossy().into_owned()));
    }
    envs
}
pub fn sanitize_binary_output(s: &str) -> String {
    s.chars()
        .filter(|c| {
            matches!(*c, '\t' | '\n' | '\r')
                || (!c.is_control() && !matches!(*c as u32, 0xfff9..=0xfffb))
        })
        .collect()
}
static TRACKED: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();
fn tracked() -> &'static Mutex<Vec<u32>> {
    TRACKED.get_or_init(|| Mutex::new(Vec::new()))
}
pub fn track_detached_child_pid(pid: u32) {
    tracked().lock().unwrap().push(pid);
}
pub fn untrack_detached_child_pid(pid: u32) {
    tracked().lock().unwrap().retain(|p| *p != pid);
}
pub fn kill_process_tree(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();
    }
}
pub fn kill_tracked_detached_children() {
    let pids = std::mem::take(&mut *tracked().lock().unwrap());
    for pid in pids {
        kill_process_tree(pid);
    }
}
#[allow(dead_code)]
fn _child_id(child: &Child) -> u32 {
    child.id()
}
