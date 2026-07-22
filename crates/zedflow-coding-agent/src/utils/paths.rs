use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default)]
pub struct PathInputOptions {
    pub trim: bool,
    pub expand_tilde: bool,
    pub strip_at_prefix: bool,
    pub normalize_unicode_spaces: bool,
}

#[must_use]
pub fn canonicalize_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[must_use]
pub fn is_local_path(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    !["npm:", "git:", "github:", "http:", "https:", "ssh:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

#[must_use]
pub fn normalize_path(input: &str) -> String {
    normalize_path_with(
        input,
        PathInputOptions {
            expand_tilde: true,
            ..Default::default()
        },
    )
}

#[must_use]
pub fn normalize_path_with(input: &str, options: PathInputOptions) -> String {
    let mut value = if options.trim {
        input.trim().to_owned()
    } else {
        input.to_owned()
    };
    if options.normalize_unicode_spaces {
        value = value
            .chars()
            .map(|c| match c {
                '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => ' ',
                c => c,
            })
            .collect();
    }
    if options.strip_at_prefix {
        value = value.strip_prefix('@').unwrap_or(&value).to_owned();
    }
    if options.expand_tilde {
        if value == "~" {
            return dirs_home();
        }
        if let Some(rest) = value.strip_prefix("~/") {
            return Path::new(&dirs_home())
                .join(rest)
                .to_string_lossy()
                .into_owned();
        }
    }
    if let Some(rest) = value.strip_prefix("file://") {
        if let Ok(decoded) = percent_decode(rest) {
            return decoded;
        }
    }
    value
}

fn dirs_home() -> String {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn percent_decode(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(());
            }
            out.push(
                u8::from_str_radix(
                    std::str::from_utf8(&bytes[i + 1..i + 3]).map_err(|_| ())?,
                    16,
                )
                .map_err(|_| ())?,
            );
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Ok(String::from_utf8(out).map_err(|_| ())?)
}

#[must_use]
pub fn resolve_path(input: &str, base_dir: impl AsRef<Path>) -> PathBuf {
    let value = normalize_path(input);
    let path = Path::new(&value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.as_ref().join(path)
    }
}

#[must_use]
pub fn get_cwd_relative_path(
    file_path: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
) -> Option<PathBuf> {
    let cwd = cwd.as_ref();
    let path = resolve_path(&file_path.as_ref().to_string_lossy(), cwd);
    let relative = path.strip_prefix(cwd).ok()?;
    Some(if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative.to_path_buf()
    })
}

#[must_use]
pub fn format_path_relative_to_cwd_or_absolute(
    file_path: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
) -> String {
    get_cwd_relative_path(&file_path, &cwd)
        .unwrap_or_else(|| resolve_path(&file_path.as_ref().to_string_lossy(), cwd))
        .to_string_lossy()
        .replace('\\', "/")
}
