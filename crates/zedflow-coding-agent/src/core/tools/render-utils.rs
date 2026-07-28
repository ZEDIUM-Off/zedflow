//! Shared formatting helpers for built-in tool renderers.

use std::path::{Path, PathBuf};

use zedflow_agent::types::{AgentToolResult, AgentToolResultContent};
use zedflow_tui::terminal_image::{
    get_capabilities, get_image_dimensions, hyperlink, image_fallback,
};

use crate::utils::{ansi::strip_ansi, shell::sanitize_binary_output};

pub fn shorten_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    home.as_deref()
        .and_then(|home| path.strip_prefix(home).ok())
        .map_or_else(
            || path.display().to_string(),
            |suffix| format!("~/{}", suffix.display()),
        )
}

pub fn link_path(styled_text: &str, raw_path: impl AsRef<Path>, cwd: impl AsRef<Path>) -> String {
    if !get_capabilities().hyperlinks {
        return styled_text.to_owned();
    }
    let raw_path = raw_path.as_ref();
    let absolute = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        cwd.as_ref().join(raw_path)
    };
    hyperlink(styled_text, &format!("file://{}", absolute.display()))
}

pub fn string_arg(value: Option<&serde_json::Value>) -> Option<&str> {
    match value {
        Some(serde_json::Value::String(value)) => Some(value),
        Some(serde_json::Value::Null) | None => Some(""),
        _ => None,
    }
}

pub fn replace_tabs(text: &str) -> String {
    text.replace('\t', "   ")
}

pub fn normalize_display_text(text: &str) -> String {
    text.replace('\r', "")
}

pub fn get_text_output<T>(result: Option<&AgentToolResult<T>>, show_images: bool) -> String {
    let Some(result) = result else {
        return String::new();
    };
    let mut text = result
        .content
        .iter()
        .filter_map(|content| match content {
            AgentToolResultContent::Text(content) => Some(normalize_display_text(
                &sanitize_binary_output(&strip_ansi(&content.text)),
            )),
            AgentToolResultContent::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !show_images || get_capabilities().images.is_none() {
        let images = result.content.iter().filter_map(|content| match content {
            AgentToolResultContent::Image(image) => Some(image_fallback(
                &image.mime_type,
                get_image_dimensions(&image.data, &image.mime_type),
                None,
            )),
            AgentToolResultContent::Text(_) => None,
        });
        for image in images {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&image);
        }
    }
    text
}

pub const INVALID_ARG_TEXT: &str = "[invalid arg]";
