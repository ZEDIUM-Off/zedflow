use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn canonicalize_path(path: impl AsRef<Path>) -> PathBuf {
    fs::canonicalize(path.as_ref()).unwrap_or_else(|_| path.as_ref().to_path_buf())
}

pub fn is_local_path(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

#[derive(Clone, Debug, Default)]
pub struct PathInputOptions {
    pub trim: bool,
    pub expand_tilde: Option<bool>,
    pub home_dir: Option<PathBuf>,
    pub strip_at_prefix: bool,
    pub normalize_unicode_spaces: bool,
}

pub fn normalize_path(input: &str, options: &PathInputOptions) -> PathBuf {
    let mut value = if options.trim {
        input.trim().to_owned()
    } else {
        input.to_owned()
    };
    if options.normalize_unicode_spaces {
        value = value.chars().map(|c| if matches!(c, '\u{a0}'..='\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}') { ' ' } else { c }).collect();
    }
    if options.strip_at_prefix {
        value = value.strip_prefix('@').unwrap_or(&value).to_owned();
    }
    if options.expand_tilde.unwrap_or(true) && (value == "~" || value.starts_with("~/")) {
        let home = options
            .home_dir
            .clone()
            .unwrap_or_else(|| env::var_os("HOME").map(PathBuf::from).unwrap_or_default());
        return if value == "~" {
            home
        } else {
            home.join(&value[2..])
        };
    }
    if let Some(rest) = value.strip_prefix("file://") {
        return PathBuf::from(rest);
    }
    PathBuf::from(value)
}

pub fn resolve_path(
    input: &str,
    base_dir: impl AsRef<Path>,
    options: &PathInputOptions,
) -> PathBuf {
    let path = normalize_path(input, options);
    if path.is_absolute() {
        path
    } else {
        base_dir.as_ref().join(path)
    }
}

pub fn get_cwd_relative_path(file_path: &Path, cwd: &Path) -> Option<PathBuf> {
    let cwd = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let path = resolve_path(
        &file_path.to_string_lossy(),
        &cwd,
        &PathInputOptions::default(),
    );
    path.strip_prefix(&cwd).ok().map(|p| {
        if p.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            p.to_path_buf()
        }
    })
}

pub fn format_path_relative_to_cwd_or_absolute(file_path: &Path, cwd: &Path) -> String {
    get_cwd_relative_path(file_path, cwd)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            resolve_path(
                &file_path.to_string_lossy(),
                cwd,
                &PathInputOptions::default(),
            )
            .to_string_lossy()
            .replace('\\', "/")
        })
}

pub fn mark_path_ignored_by_cloud_sync(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        for attr in ["com.dropbox.ignored", "com.apple.fileprovider.ignore#P"] {
            let _ = std::process::Command::new("xattr")
                .args(["-w", attr, "1"])
                .arg(path)
                .status();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("setfattr")
            .args(["-n", "user.com.dropbox.ignored", "-v", "1"])
            .arg(path)
            .status();
    }
    Ok(())
}
