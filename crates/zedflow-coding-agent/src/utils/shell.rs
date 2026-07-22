use std::env;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellConfig {
    pub shell: String,
    pub args: Vec<String>,
    pub command_transport: Option<String>,
}

#[must_use]
pub fn get_shell_config(custom: Option<&str>) -> Result<ShellConfig, String> {
    if let Some(path) = custom {
        if Path::new(path).exists() {
            return Ok(ShellConfig {
                shell: path.into(),
                args: vec!["-c".into()],
                command_transport: None,
            });
        }
        return Err(format!("Custom shell path not found: {path}"));
    }
    #[cfg(windows)]
    {
        for key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = env::var_os(key) {
                let p = Path::new(&base).join("Git/bin/bash.exe");
                if p.exists() {
                    return Ok(ShellConfig {
                        shell: p.to_string_lossy().into(),
                        args: vec!["-c".into()],
                        command_transport: None,
                    });
                }
            }
        }
    }
    for shell in ["/bin/bash", "bash", "sh"] {
        if shell == "sh"
            || Path::new(shell).exists()
            || Command::new("which")
                .arg(shell)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        {
            return Ok(ShellConfig {
                shell: shell.into(),
                args: vec!["-c".into()],
                command_transport: None,
            });
        }
    }
    Err("No shell found".into())
}

#[must_use]
pub fn get_shell_env(bin_dir: Option<&Path>) -> Vec<(String, String)> {
    let mut envs: Vec<_> = env::vars().collect();
    if let Some(bin) = bin_dir {
        let key = envs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
            .map(|(k, _)| k.clone())
            .unwrap_or_else(|| "PATH".into());
        if let Some((_, value)) = envs.iter_mut().find(|(k, _)| *k == key) {
            let mut paths: Vec<_> = env::split_paths(value).collect();
            if !paths.iter().any(|path| path == bin) {
                paths.insert(0, bin.to_path_buf());
            }
            *value = env::join_paths(paths)
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
        }
    }
    envs
}

#[must_use]
pub fn sanitize_binary_output(value: &str) -> String {
    value
        .chars()
        .filter(|c| {
            matches!(*c, '\t' | '\n' | '\r')
                || (!c.is_control() && !matches!(*c as u32, 0xfff9..=0xfffb))
        })
        .collect()
}
