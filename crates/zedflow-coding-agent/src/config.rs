use std::{
    env,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub const PACKAGE_NAME: &str = "@earendil-works/pi-coding-agent";
pub const APP_NAME: &str = "pi";
pub const APP_TITLE: &str = "π";
pub const CONFIG_DIR_NAME: &str = ".pi";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ENV_AGENT_DIR: &str = "PI_CODING_AGENT_DIR";
pub const ENV_SESSION_DIR: &str = "PI_CODING_AGENT_SESSION_DIR";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallMethod {
    BunBinary,
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Unknown,
}

pub fn detect_install_method() -> InstallMethod {
    detect_install_method_from_paths(
        &get_package_dir(),
        &env::current_exe().unwrap_or_default(),
        false,
        false,
    )
}

pub fn detect_install_method_from_paths(
    package_dir: &Path,
    executable_path: &Path,
    is_bun_binary: bool,
    is_bun_runtime: bool,
) -> InstallMethod {
    if is_bun_binary {
        return InstallMethod::BunBinary;
    }

    let resolved_path = format!("{}\0{}", package_dir.display(), executable_path.display())
        .to_lowercase()
        .replace('\\', "/");

    if resolved_path.contains("/pnpm/") || resolved_path.contains("/.pnpm/") {
        InstallMethod::Pnpm
    } else if resolved_path.contains("/yarn/") || resolved_path.contains("/.yarn/") {
        InstallMethod::Yarn
    } else if is_bun_runtime || resolved_path.contains("/install/global/node_modules/") {
        InstallMethod::Bun
    } else if resolved_path.contains("/npm/") || resolved_path.contains("/node_modules/") {
        InstallMethod::Npm
    } else {
        InstallMethod::Unknown
    }
}

pub fn read_command_output(
    command: &str,
    args: &[&str],
    require_success: bool,
) -> Result<Option<String>, String> {
    let result = Command::new(command).args(args).output();
    let output = match result {
        Ok(output) => output,
        Err(error) if require_success => {
            return Err(format!(
                "Failed to run {}: {error}",
                std::iter::once(command)
                    .chain(args.iter().copied())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        Err(_) => return Ok(None),
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        return Ok((!stdout.is_empty()).then_some(stdout));
    }
    if require_success {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let reason = if stderr.is_empty() {
            output.status.code().map_or_else(
                || "exit code unknown".to_owned(),
                |code| format!("exit code {code}"),
            )
        } else {
            stderr
        };
        return Err(format!(
            "Failed to run {}: {reason}",
            std::iter::once(command)
                .chain(args.iter().copied())
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    Ok(None)
}

pub fn get_global_package_roots(
    method: InstallMethod,
    npm_command: Option<&[String]>,
) -> Result<Vec<PathBuf>, String> {
    let read = |command: &str, args: &[&str], required| {
        read_command_output(command, args, required).map(|path| path.map(PathBuf::from))
    };
    match method {
        InstallMethod::Npm => {
            let configured = npm_command.is_some_and(|command| !command.is_empty());
            let command = npm_command
                .and_then(|command| command.first())
                .map_or("npm", String::as_str);
            let npm_args: Vec<&str> = npm_command
                .map(|command| command.iter().skip(1).map(String::as_str).collect())
                .unwrap_or_default();
            if configured && command == "bun" {
                let mut args = npm_args;
                args.extend(["pm", "bin", "-g"]);
                let bun_bin = read(command, &args, true)?;
                let mut roots = vec![home_dir().join(".bun/install/global/node_modules")];
                if let Some(bin) = bun_bin.and_then(|path| path.parent().map(Path::to_path_buf)) {
                    roots.push(bin.join("install/global/node_modules"));
                }
                return Ok(roots);
            }
            let mut args = npm_args;
            args.extend(["root", "-g"]);
            let mut roots: Vec<PathBuf> = read(command, &args, configured)?.into_iter().collect();
            if !configured {
                if let Some(prefix) = inferred_npm_prefix(&get_package_dir()) {
                    roots.push(prefix.join("lib/node_modules"));
                }
            }
            Ok(roots)
        }
        InstallMethod::Pnpm => {
            if let Some(root) = read("pnpm", &["root", "-g"], false)? {
                let parent = root.parent().map(Path::to_path_buf);
                return Ok(std::iter::once(root).chain(parent).collect());
            }
            Ok(infer_pnpm_global_root(&get_package_dir())
                .into_iter()
                .collect())
        }
        InstallMethod::Yarn => Ok(match read("yarn", &["global", "dir"], false)? {
            Some(dir) => vec![dir.clone(), dir.join("node_modules")],
            None => vec![],
        }),
        InstallMethod::Bun => {
            let bun_bin = read("bun", &["pm", "bin", "-g"], false)?;
            let mut roots = vec![home_dir().join(".bun/install/global/node_modules")];
            if let Some(bin) = bun_bin.and_then(|path| path.parent().map(Path::to_path_buf)) {
                roots.push(bin.join("install/global/node_modules"));
            }
            Ok(roots)
        }
        InstallMethod::BunBinary | InstallMethod::Unknown => Ok(vec![]),
    }
}

pub fn infer_pnpm_global_root(package_dir: &Path) -> Option<PathBuf> {
    let path = package_dir.to_string_lossy().replace('\\', "/");
    let end = path.find("/.pnpm/")?;
    let prefix = &path[..end];
    let global = prefix.rfind("/global/")?;
    let version = &prefix[global + "/global/".len()..];
    (!version.is_empty() && !version.contains('/')).then(|| PathBuf::from(prefix))
}

pub fn get_update_instruction_for_method(method: InstallMethod, package_name: &str) -> String {
    let command = match method {
        InstallMethod::Pnpm => {
            format!("pnpm install -g --ignore-scripts --config.minimumReleaseAge=0 {package_name}")
        }
        InstallMethod::Yarn => {
            format!("yarn global add --ignore-scripts {package_name}")
        }
        InstallMethod::Bun => {
            format!("bun install -g --ignore-scripts --minimum-release-age=0 {package_name}")
        }
        InstallMethod::Npm => {
            let prefix = inferred_npm_prefix(&get_package_dir())
                .map(|prefix| format!(" --prefix {}", quote_display_arg(prefix)))
                .unwrap_or_default();
            format!("npm{prefix} install -g --ignore-scripts --min-release-age=0 {package_name}")
        }
        InstallMethod::BunBinary => {
            return "Download from: https://github.com/earendil-works/pi-mono/releases/latest"
                .to_owned();
        }
        InstallMethod::Unknown => {
            return format!(
                "Update {package_name} using the package manager, wrapper, or source checkout that provides this installation."
            );
        }
    };
    format!("Run: {command}")
}

fn quote_display_arg(path: &Path) -> String {
    let path = path.to_string_lossy();
    if path.chars().any(|character| {
        matches!(
            character,
            '\u{0009}'..='\u{000d}'
                | '\u{0020}'
                | '\u{00a0}'
                | '\u{1680}'
                | '\u{2000}'..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
        )
    }) {
        format!("\"{path}\"")
    } else {
        path.into_owned()
    }
}

fn inferred_npm_prefix(package_dir: &Path) -> Option<&Path> {
    let parent = package_dir.parent()?;
    let root = if parent.file_name()?.to_string_lossy().starts_with('@')
        && parent.parent()?.file_name()? == "node_modules"
    {
        parent.parent()?
    } else if parent.file_name()? == "node_modules" {
        parent
    } else {
        return None;
    };
    let lib = root.parent()?;
    (lib.file_name()? == "lib").then(|| lib.parent()).flatten()
}

pub fn expand_tilde_path(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path
        .strip_prefix("~/")
        .or_else(|| cfg!(windows).then(|| path.strip_prefix("~\\")).flatten())
    {
        return home_dir().join(rest);
    }
    PathBuf::from(path)
}

pub fn get_package_dir() -> PathBuf {
    env::var("PI_PACKAGE_DIR")
        .ok()
        .filter(|path| !path.is_empty())
        .map(|path| normalize_path(&path))
        .unwrap_or_else(|| {
            env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
        })
}

/// Finds the package root containing the process entrypoint, if one is present.
pub fn get_entrypoint_package_dir() -> Option<PathBuf> {
    env::args_os()
        .nth(1)
        .and_then(|entrypoint| find_entrypoint_package_dir(entrypoint))
}

pub fn find_entrypoint_package_dir(entrypoint: impl AsRef<Path>) -> Option<PathBuf> {
    let mut dir = entrypoint
        .as_ref()
        .parent()
        .unwrap_or_else(|| Path::new("."));
    while dir.parent().is_some_and(|parent| parent != dir) {
        if dir.join("package.json").exists() {
            return Some(dir.to_owned());
        }
        dir = dir.parent().expect("checked above");
    }
    None
}

fn normalize_path(path: &str) -> PathBuf {
    if path.starts_with("file://") {
        return reqwest::Url::parse(path)
            .expect("PI_PACKAGE_DIR must be a valid file URL")
            .to_file_path()
            .expect("PI_PACKAGE_DIR file URL must identify a local path");
    }
    expand_tilde_path(path)
}

pub fn get_themes_dir() -> PathBuf {
    get_package_dir().join("theme")
}

pub fn get_export_template_dir() -> PathBuf {
    get_package_dir().join("export-html")
}

pub fn get_package_json_path() -> PathBuf {
    get_package_dir().join("package.json")
}

pub fn get_readme_path() -> PathBuf {
    absolute_package_asset("README.md")
}

pub fn get_docs_path() -> PathBuf {
    absolute_package_asset("docs")
}

pub fn get_examples_path() -> PathBuf {
    absolute_package_asset("examples")
}

pub fn get_changelog_path() -> PathBuf {
    absolute_package_asset("CHANGELOG.md")
}

pub fn get_interactive_assets_dir() -> PathBuf {
    get_package_dir().join("assets")
}

pub fn get_bundled_interactive_asset_path(name: &str) -> PathBuf {
    get_interactive_assets_dir().join(name)
}

pub fn get_share_viewer_url(gist_id: &str) -> String {
    let base = env::var("PI_SHARE_VIEWER_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| "https://pi.dev/session/".to_owned());
    format!("{base}#{gist_id}")
}

pub fn get_agent_dir() -> PathBuf {
    env::var(ENV_AGENT_DIR)
        .ok()
        .filter(|path| !path.is_empty())
        .map(|path| expand_tilde_path(&path))
        .unwrap_or_else(|| home_dir().join(CONFIG_DIR_NAME).join("agent"))
}

pub fn get_custom_themes_dir() -> PathBuf {
    get_agent_dir().join("themes")
}

pub fn get_models_path() -> PathBuf {
    get_agent_dir().join("models.json")
}

pub fn get_auth_path() -> PathBuf {
    get_agent_dir().join("auth.json")
}

pub fn get_settings_path() -> PathBuf {
    get_agent_dir().join("settings.json")
}

pub fn get_tools_dir() -> PathBuf {
    get_agent_dir().join("tools")
}

pub fn get_bin_dir() -> PathBuf {
    get_agent_dir().join("bin")
}

pub fn get_prompts_dir() -> PathBuf {
    get_agent_dir().join("prompts")
}

pub fn get_sessions_dir() -> PathBuf {
    get_agent_dir().join("sessions")
}

pub fn get_debug_log_path() -> PathBuf {
    get_agent_dir().join(format!("{APP_NAME}-debug.log"))
}

fn absolute_package_asset(name: &str) -> PathBuf {
    let path = get_package_dir().join(name);
    lexical_normalize(if path.is_absolute() {
        path
    } else {
        env::current_dir().unwrap_or_default().join(path)
    })
}

pub fn normalize_existing_path_for_comparison(
    path: impl AsRef<Path>,
    resolve_symlinks: bool,
) -> Option<PathBuf> {
    let path = path.as_ref();
    let resolved = lexical_normalize(if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir().ok()?.join(path)
    });
    if !resolved.exists() {
        return None;
    }

    let normalized = if resolve_symlinks {
        resolved.canonicalize().ok()?
    } else {
        resolved
    };
    #[cfg(windows)]
    return Some(PathBuf::from(normalized.to_string_lossy().to_lowercase()));
    #[cfg(not(windows))]
    Some(normalized)
}

pub fn get_path_comparison_candidates(path: impl AsRef<Path>) -> Vec<PathBuf> {
    let path = path.as_ref();
    let mut candidates = Vec::new();
    for candidate in [
        normalize_existing_path_for_comparison(path, false),
        normalize_existing_path_for_comparison(path, true),
    ]
    .into_iter()
    .flatten()
    {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

fn lexical_normalize(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn home_dir() -> PathBuf {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = env::var_os("HOME");

    home.map(PathBuf::from)
        .or_else(|| {
            #[allow(deprecated)]
            env::home_dir()
        })
        .unwrap_or_default()
}
