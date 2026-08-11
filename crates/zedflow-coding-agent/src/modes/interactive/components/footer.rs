//! Pure footer formatting and rendering for Pi's interactive chrome.

use std::path::{Component as PathComponent, Path, PathBuf};
use zedflow_tui::{Component, truncate_to_width, visible_width};

#[must_use]
pub fn sanitize_status_text(text: &str) -> String {
    text.replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[must_use]
pub fn format_tokens(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..10_000 => format!("{:.1}k", count as f64 / 1_000.0),
        10_000..1_000_000 => format!("{}k", ((count as f64) / 1_000.0).round() as u64),
        1_000_000..10_000_000 => format!("{:.1}M", count as f64 / 1_000_000.0),
        _ => format!("{}M", ((count as f64) / 1_000_000.0).round() as u64),
    }
}

#[must_use]
pub fn format_cwd_for_footer(cwd: &Path, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return cwd.display().to_string();
    };
    let resolved_cwd = absolute_normalized(cwd);
    let resolved_home = absolute_normalized(home);
    match resolved_cwd.strip_prefix(&resolved_home) {
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
            PathComponent::ParentDir => {
                result.pop();
            }
            PathComponent::CurDir => {}
            _ => result.push(part.as_os_str()),
        }
        result
    })
}

#[derive(Debug, Clone, Default)]
pub struct FooterSnapshot {
    pub cwd: String,
    pub git_branch: Option<String>,
    pub session_name: Option<String>,
    pub stats: Vec<String>,
    pub model: String,
    pub provider: Option<String>,
    pub available_provider_count: usize,
    pub extension_statuses: Vec<(String, String)>,
}

impl Component for FooterSnapshot {
    fn render(&self, width: usize) -> Vec<String> {
        let mut pwd = self.cwd.clone();
        if let Some(branch) = &self.git_branch {
            pwd.push_str(&format!(" ({branch})"));
        }
        if let Some(name) = &self.session_name {
            pwd.push_str(&format!(" • {name}"));
        }
        let pwd = truncate_to_width(&pwd, width, "...", false);

        let mut left = self.stats.join(" ");
        if visible_width(&left) > width {
            left = truncate_to_width(&left, width, "...", false);
        }
        let plain_right = if self.model.is_empty() {
            "no-model"
        } else {
            &self.model
        };
        let with_provider = self
            .provider
            .as_ref()
            .filter(|_| self.available_provider_count > 1)
            .map(|provider| format!("({provider}) {plain_right}"));
        let right = with_provider
            .filter(|right| visible_width(&left) + 2 + visible_width(right) <= width)
            .unwrap_or_else(|| plain_right.into());
        let available = width.saturating_sub(visible_width(&left) + 2);
        let right = truncate_to_width(&right, available, "", false);
        let padding = width.saturating_sub(visible_width(&left) + visible_width(&right));
        let stats = if right.is_empty() {
            left
        } else {
            format!("{left}{}{right}", " ".repeat(padding))
        };
        let mut lines = vec![pwd, stats];

        if !self.extension_statuses.is_empty() {
            let mut statuses = self.extension_statuses.clone();
            statuses.sort_by(|a, b| a.0.cmp(&b.0));
            let line = statuses
                .into_iter()
                .map(|(_, text)| sanitize_status_text(&text))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(truncate_to_width(&line, width, "...", false));
        }
        lines
    }
}
