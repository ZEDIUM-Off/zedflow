use std::{env, path::PathBuf};

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
    if let Some(rest) = path.strip_prefix("~/") {
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

pub fn get_readme_path() -> PathBuf {
    absolute_package_asset("README.md")
}

pub fn get_docs_path() -> PathBuf {
    absolute_package_asset("docs")
}

pub fn get_examples_path() -> PathBuf {
    absolute_package_asset("examples")
}

pub fn get_agent_dir() -> PathBuf {
    env::var(ENV_AGENT_DIR)
        .ok()
        .filter(|path| !path.is_empty())
        .map(|path| expand_tilde_path(&path))
        .unwrap_or_else(|| home_dir().join(CONFIG_DIR_NAME).join("agent"))
}

fn absolute_package_asset(name: &str) -> PathBuf {
    let path = get_package_dir().join(name);
    if path.is_absolute() {
        path
    } else {
        env::current_dir().unwrap_or_default().join(path)
    }
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
