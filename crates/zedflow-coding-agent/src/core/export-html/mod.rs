#[path = "ansi-to-html.rs"]
pub mod ansi_to_html;
#[path = "tool-renderer.rs"]
pub mod tool_renderer;

pub use ansi_to_html::{ansi_lines_to_html, ansi_to_html};
pub use tool_renderer::{
    RenderedToolHtml, ToolHtmlRenderer, ToolHtmlRendererDeps, create_tool_html_renderer,
};

#[must_use]
pub fn export_session_to_html(content: &str) -> String {
    format!(
        "<!doctype html><html><body><pre>{}</pre></body></html>",
        ansi_to_html(content)
    )
}
