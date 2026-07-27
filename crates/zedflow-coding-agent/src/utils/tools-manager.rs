use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub async fn ensure_tool(tool: &'static str, silent: bool) -> Option<PathBuf> {
    tokio::task::spawn_blocking(move || ensure_tool_blocking(tool, silent))
        .await
        .ok()
        .flatten()
}

fn ensure_tool_blocking(tool: &str, silent: bool) -> Option<PathBuf> {
    let (name, system_names) = match tool {
        "fd" => ("fd", &["fd", "fdfind"][..]),
        "rg" => ("ripgrep", &["rg"][..]),
        _ => return None,
    };

    if let Some(path) = managed_binary_path(tool)
        && path.exists()
    {
        return Some(path);
    }
    if let Some(command) = system_names
        .iter()
        .find(|command| Command::new(command).arg("--version").output().is_ok())
    {
        return Some(PathBuf::from(command));
    }

    if offline_mode() {
        if !silent {
            eprintln!("{name} not found. Offline mode enabled, skipping download.");
        }
        return None;
    }
    if env::consts::OS == "android" {
        if !silent {
            let package = if tool == "rg" { "ripgrep" } else { tool };
            eprintln!("{name} not found. Install with: pkg install {package}");
        }
        return None;
    }

    if !silent {
        eprintln!("{name} not found. Downloading...");
    }
    match download_tool(tool) {
        Ok(path) => {
            if !silent {
                eprintln!("{name} installed to {}", path.display());
            }
            Some(path)
        }
        Err(error) => {
            if !silent {
                eprintln!("Failed to download {name}: {error}");
            }
            None
        }
    }
}

fn offline_mode() -> bool {
    env::var("PI_OFFLINE")
        .is_ok_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

fn managed_binary_path(tool: &str) -> Option<PathBuf> {
    let agent_dir = if let Some(configured) = env::var_os("PI_CODING_AGENT_DIR") {
        crate::path_utils::expand_path(&configured.to_string_lossy()).ok()?
    } else {
        PathBuf::from(env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?).join(".pi/agent")
    };
    Some(
        agent_dir
            .join("bin")
            .join(format!("{tool}{}", env::consts::EXE_SUFFIX)),
    )
}

fn download_tool(tool: &str) -> Result<PathBuf, String> {
    let (repo, tag_prefix) = match tool {
        "fd" => ("sharkdp/fd", "v"),
        "rg" => ("BurntSushi/ripgrep", ""),
        _ => return Err(format!("Unknown tool: {tool}")),
    };
    let binary_path = managed_binary_path(tool).ok_or("Home directory is unavailable")?;
    let tools_dir = binary_path.parent().expect("managed binary has parent");

    let api = reqwest::blocking::Client::builder()
        .timeout(NETWORK_TIMEOUT)
        .user_agent("pi-coding-agent")
        .build()
        .map_err(|error| error.to_string())?;
    let body = api
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::text)
        .map_err(|error| error.to_string())?;
    let release: serde_yaml::Value =
        serde_yaml::from_str(&body).map_err(|error| error.to_string())?;
    let mut version = release
        .get("tag_name")
        .and_then(serde_yaml::Value::as_str)
        .ok_or("GitHub release omitted tag_name")?
        .trim_start_matches('v')
        .to_owned();
    if tool == "fd" && env::consts::OS == "macos" && env::consts::ARCH == "x86_64" {
        version = "10.3.0".to_owned();
    }
    let asset = asset_name(tool, &version)?;
    let url = format!("https://github.com/{repo}/releases/download/{tag_prefix}{version}/{asset}");
    let bytes = reqwest::blocking::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::bytes)
        .map_err(|error| error.to_string())?;

    fs::create_dir_all(tools_dir).map_err(|error| error.to_string())?;
    let archive_path = tools_dir.join(&asset);
    fs::write(&archive_path, bytes).map_err(|error| error.to_string())?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let extract_dir = tools_dir.join(format!(
        "extract_tmp_{tool}_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&extract_dir).map_err(|error| error.to_string())?;

    let result = (|| {
        extract_archive(&archive_path, &extract_dir, &asset)?;
        let binary_name = format!("{tool}{}", env::consts::EXE_SUFFIX);
        let extracted = find_binary(&extract_dir, OsStr::new(&binary_name))?
            .ok_or_else(|| format!("Binary not found in archive: expected {binary_name}"))?;
        fs::rename(extracted, &binary_path).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary_path, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        Ok(binary_path.clone())
    })();
    let _ = fs::remove_file(archive_path);
    let _ = fs::remove_dir_all(extract_dir);
    result
}

fn asset_name(tool: &str, version: &str) -> Result<String, String> {
    let arch = if env::consts::ARCH == "aarch64" {
        "aarch64"
    } else {
        "x86_64"
    };
    match (tool, env::consts::OS) {
        ("fd", "macos") => Ok(format!("fd-v{version}-{arch}-apple-darwin.tar.gz")),
        ("fd", "linux") => Ok(format!("fd-v{version}-{arch}-unknown-linux-gnu.tar.gz")),
        ("fd", "windows") => Ok(format!("fd-v{version}-{arch}-pc-windows-msvc.zip")),
        ("rg", "macos") => Ok(format!("ripgrep-{version}-{arch}-apple-darwin.tar.gz")),
        ("rg", "linux") if arch == "aarch64" => Ok(format!(
            "ripgrep-{version}-aarch64-unknown-linux-gnu.tar.gz"
        )),
        ("rg", "linux") => Ok(format!(
            "ripgrep-{version}-x86_64-unknown-linux-musl.tar.gz"
        )),
        ("rg", "windows") => Ok(format!("ripgrep-{version}-{arch}-pc-windows-msvc.zip")),
        _ => Err(format!(
            "Unsupported platform: {}/{}",
            env::consts::OS,
            env::consts::ARCH
        )),
    }
}

fn extract_archive(archive: &Path, destination: &Path, asset: &str) -> Result<(), String> {
    if asset.ends_with(".tar.gz") {
        return run(
            "tar",
            [
                OsStr::new("xzf"),
                archive.as_os_str(),
                OsStr::new("-C"),
                destination.as_os_str(),
            ],
        );
    }
    if !asset.ends_with(".zip") {
        return Err(format!("Unsupported archive format: {asset}"));
    }

    #[cfg(windows)]
    {
        let tar = env::var_os("SystemRoot")
            .or_else(|| env::var_os("WINDIR"))
            .map(|root| PathBuf::from(root).join("System32/tar.exe"))
            .filter(|path| path.exists())
            .unwrap_or_else(|| PathBuf::from("tar.exe"));
        if run(
            &tar,
            [
                OsStr::new("xf"),
                archive.as_os_str(),
                OsStr::new("-C"),
                destination.as_os_str(),
            ],
        )
        .is_ok()
        {
            return Ok(());
        }
        return run(
            "powershell.exe",
            [
                OsStr::new("-NoLogo"),
                OsStr::new("-NoProfile"),
                OsStr::new("-NonInteractive"),
                OsStr::new("-Command"),
                OsStr::new(
                    "& { param($archive, $destination) $ErrorActionPreference = 'Stop'; Expand-Archive -LiteralPath $archive -DestinationPath $destination -Force }",
                ),
                archive.as_os_str(),
                destination.as_os_str(),
            ],
        );
    }
    #[cfg(not(windows))]
    {
        if run(
            "unzip",
            [
                OsStr::new("-q"),
                archive.as_os_str(),
                OsStr::new("-d"),
                destination.as_os_str(),
            ],
        )
        .is_ok()
        {
            return Ok(());
        }
        run(
            "tar",
            [
                OsStr::new("xf"),
                archive.as_os_str(),
                OsStr::new("-C"),
                destination.as_os_str(),
            ],
        )
    }
}

fn run<I, S>(command: impl AsRef<OsStr>, arguments: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(command)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(if message.is_empty() {
        format!("exit status {}", output.status)
    } else {
        message
    })
}

fn find_binary(root: &Path, binary_name: &OsStr) -> Result<Option<PathBuf>, String> {
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_file() && entry.file_name() == binary_name {
                return Ok(Some(entry.path()));
            }
            if file_type.is_dir() {
                directories.push(entry.path());
            }
        }
    }
    Ok(None)
}
