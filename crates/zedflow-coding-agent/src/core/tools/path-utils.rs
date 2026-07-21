//! Path normalization and read-path fallbacks used by coding-agent tools.

use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

const NARROW_NO_BREAK_SPACE: char = '\u{202f}';

fn normalize_input(input: &str, special: bool) -> io::Result<PathBuf> {
    let mut value = if special {
        input
            .chars()
            .map(|character| {
                if matches!(
                    character,
                    '\u{00a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
                ) {
                    ' '
                } else {
                    character
                }
            })
            .collect::<String>()
    } else {
        input.to_owned()
    };

    if special && value.starts_with('@') {
        value.remove(0);
    }

    if value == "~" || value.starts_with("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "home directory is unavailable")
            })?;
        return Ok(if value == "~" {
            PathBuf::from(home)
        } else {
            PathBuf::from(home).join(&value[2..])
        });
    }

    if let Some(encoded) = value.strip_prefix("file://") {
        let encoded = encoded.strip_prefix("localhost").unwrap_or(encoded);
        if !encoded.starts_with('/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file URL host must be empty or localhost",
            ));
        }
        return Ok(PathBuf::from(percent_decode(encoded)?));
    }

    Ok(PathBuf::from(value))
}

pub fn path_exists(file_path: impl AsRef<Path>) -> bool {
    file_path.as_ref().try_exists().unwrap_or(false)
}

pub async fn path_exists_async(file_path: impl AsRef<Path>) -> bool {
    tokio::fs::metadata(file_path.as_ref()).await.is_ok()
}

pub fn expand_path(file_path: &str) -> io::Result<PathBuf> {
    normalize_input(file_path, true)
}

pub fn resolve_to_cwd(file_path: &str, cwd: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = normalize_input(file_path, true)?;
    let base = normalize_input(&cwd.as_ref().to_string_lossy(), false)?;
    std::path::absolute(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

fn macos_screenshot_variant(path: &Path) -> PathBuf {
    static AM_PM: OnceLock<Regex> = OnceLock::new();
    let regex = AM_PM.get_or_init(|| Regex::new(r"(?i) (AM|PM)\.").expect("valid AM/PM regex"));
    PathBuf::from(
        regex
            .replace_all(
                &path.to_string_lossy(),
                format!("{NARROW_NO_BREAK_SPACE}$1."),
            )
            .into_owned(),
    )
}

fn percent_decode(value: &str) -> io::Result<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 16).ok())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid file URL escape")
                })?;
            if matches!(hex, b'/' | b'\\' | 0) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "encoded path separator or NUL in file URL",
                ));
            }
            decoded.push(hex);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

fn nfd_variant(path: &Path) -> PathBuf {
    let mut decomposed = String::new();
    for character in path.to_string_lossy().chars() {
        if let Some(replacement) = latin_nfd(character) {
            decomposed.push_str(replacement);
        } else {
            decomposed.push(character);
        }
    }
    PathBuf::from(decomposed)
}

fn latin_nfd(character: char) -> Option<&'static str> {
    Some(match character {
        'À' => "A\u{300}",
        'Á' => "A\u{301}",
        'Â' => "A\u{302}",
        'Ã' => "A\u{303}",
        'Ä' => "A\u{308}",
        'Å' => "A\u{30a}",
        'Ç' => "C\u{327}",
        'È' => "E\u{300}",
        'É' => "E\u{301}",
        'Ê' => "E\u{302}",
        'Ë' => "E\u{308}",
        'Ì' => "I\u{300}",
        'Í' => "I\u{301}",
        'Î' => "I\u{302}",
        'Ï' => "I\u{308}",
        'Ñ' => "N\u{303}",
        'Ò' => "O\u{300}",
        'Ó' => "O\u{301}",
        'Ô' => "O\u{302}",
        'Õ' => "O\u{303}",
        'Ö' => "O\u{308}",
        'Ù' => "U\u{300}",
        'Ú' => "U\u{301}",
        'Û' => "U\u{302}",
        'Ü' => "U\u{308}",
        'Ý' => "Y\u{301}",
        'à' => "a\u{300}",
        'á' => "a\u{301}",
        'â' => "a\u{302}",
        'ã' => "a\u{303}",
        'ä' => "a\u{308}",
        'å' => "a\u{30a}",
        'ç' => "c\u{327}",
        'è' => "e\u{300}",
        'é' => "e\u{301}",
        'ê' => "e\u{302}",
        'ë' => "e\u{308}",
        'ì' => "i\u{300}",
        'í' => "i\u{301}",
        'î' => "i\u{302}",
        'ï' => "i\u{308}",
        'ñ' => "n\u{303}",
        'ò' => "o\u{300}",
        'ó' => "o\u{301}",
        'ô' => "o\u{302}",
        'õ' => "o\u{303}",
        'ö' => "o\u{308}",
        'ù' => "u\u{300}",
        'ú' => "u\u{301}",
        'û' => "u\u{302}",
        'ü' => "u\u{308}",
        'ý' => "y\u{301}",
        'ÿ' => "y\u{308}",
        _ => return None,
    })
}

fn curly_quote_variant(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\'', "\u{2019}"))
}

pub fn resolve_read_path(file_path: &str, cwd: impl AsRef<Path>) -> io::Result<PathBuf> {
    let resolved = resolve_to_cwd(file_path, cwd)?;
    if path_exists(&resolved) {
        return Ok(resolved);
    }

    let am_pm = macos_screenshot_variant(&resolved);
    if am_pm != resolved && path_exists(&am_pm) {
        return Ok(am_pm);
    }

    let nfd = nfd_variant(&resolved);
    if nfd != resolved && path_exists(&nfd) {
        return Ok(nfd);
    }

    let curly = curly_quote_variant(&resolved);
    if curly != resolved && path_exists(&curly) {
        return Ok(curly);
    }

    let nfd_curly = curly_quote_variant(&nfd);
    if nfd_curly != resolved && path_exists(&nfd_curly) {
        return Ok(nfd_curly);
    }

    Ok(resolved)
}

pub async fn resolve_read_path_async(
    file_path: &str,
    cwd: impl AsRef<Path>,
) -> io::Result<PathBuf> {
    let resolved = resolve_to_cwd(file_path, cwd)?;
    if path_exists_async(&resolved).await {
        return Ok(resolved);
    }

    let am_pm = macos_screenshot_variant(&resolved);
    if am_pm != resolved && path_exists_async(&am_pm).await {
        return Ok(am_pm);
    }

    let nfd = nfd_variant(&resolved);
    if nfd != resolved && path_exists_async(&nfd).await {
        return Ok(nfd);
    }

    let curly = curly_quote_variant(&resolved);
    if curly != resolved && path_exists_async(&curly).await {
        return Ok(curly);
    }

    let nfd_curly = curly_quote_variant(&nfd);
    if nfd_curly != resolved && path_exists_async(&nfd_curly).await {
        return Ok(nfd_curly);
    }

    Ok(resolved)
}
