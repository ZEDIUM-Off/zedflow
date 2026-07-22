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
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
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
    package_source_asset(&["modes", "interactive", "theme"])
}

pub fn get_export_template_dir() -> PathBuf {
    package_source_asset(&["core", "export-html"])
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
    package_source_asset(&["modes", "interactive", "assets"])
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

fn package_source_asset(components: &[&str]) -> PathBuf {
    let package_dir = get_package_dir();
    let mut path = package_dir.join(if package_dir.join("src").exists() {
        "src"
    } else {
        "dist"
    });
    path.extend(components);
    path
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
