use std::{
    env,
    path::{Component, Path, PathBuf},
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
    if path.chars().any(char::is_whitespace) {
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
