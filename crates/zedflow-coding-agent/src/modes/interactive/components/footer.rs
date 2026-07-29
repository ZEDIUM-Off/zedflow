//! Pure footer formatting helpers shared by the interactive renderer.

use std::path::{Component, Path, PathBuf};

/// Remove control whitespace from extension status text, matching Pi's one-line footer.
#[must_use]
pub fn sanitize_status_text(text: &str) -> String {
    text.replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format token counts for Pi's compact footer display.
#[must_use]
pub fn format_tokens(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..10_000 => format!("{:.1}k", count as f64 / 1_000.0),
        10_000..1_000_000 => format!("{}k", count / 1_000),
        1_000_000..10_000_000 => format!("{:.1}M", count as f64 / 1_000_000.0),
        _ => format!("{}M", count / 1_000_000),
    }
}

/// Replace a cwd under `home` with Pi's `~` display form.
#[must_use]
pub fn format_cwd_for_footer(cwd: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return cwd.display().to_string();
    };
    let cwd = absolute_normalized(cwd);
    let home = absolute_normalized(home);
    match cwd.strip_prefix(&home) {
        Ok(path) if path.as_os_str().is_empty() => "~".into(),
        Ok(path) => format!("~{}{}", std::path::MAIN_SEPARATOR, path.display()),
        Err(_) => cwd.display().to_string(),
    }
}

fn absolute_normalized(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    path.components().fold(PathBuf::new(), |mut result, part| {
        match part {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            _ => result.push(part.as_os_str()),
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_text_matches_pi_compaction_and_home_rules() {
        assert_eq!(sanitize_status_text(" a\n\t b  "), "a b");
        assert_eq!(format_tokens(9_999), "10.0k");
        assert_eq!(format_tokens(10_000), "10k");
        assert_eq!(
            format_cwd_for_footer(Path::new("/home/a/project"), Some(Path::new("/home/a"))),
            "~/project"
        );
        assert_eq!(
            format_cwd_for_footer(Path::new("/home/a/../other"), Some(Path::new("/home/a"))),
            "/home/other"
        );
    }
}
