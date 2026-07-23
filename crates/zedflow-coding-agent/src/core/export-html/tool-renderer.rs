use super::ansi_to_html::ansi_lines_to_html;

#[derive(Debug, Clone, Default)]
pub struct ToolHtmlRendererDeps {
    pub width: usize,
}
#[derive(Debug, Clone, Default)]
pub struct RenderedToolHtml {
    pub collapsed: Option<String>,
    pub expanded: Option<String>,
}
#[derive(Debug, Clone, Default)]
pub struct ToolHtmlRenderer;
impl ToolHtmlRenderer {
    #[must_use]
    pub fn render_call(&self, lines: &[String]) -> String {
        ansi_lines_to_html(lines)
    }
    #[must_use]
    pub fn render_result(&self, collapsed: &[String], expanded: &[String]) -> RenderedToolHtml {
        RenderedToolHtml {
            collapsed: Some(ansi_lines_to_html(collapsed)),
            expanded: Some(ansi_lines_to_html(expanded)),
        }
    }
}
#[must_use]
pub fn create_tool_html_renderer(_deps: ToolHtmlRendererDeps) -> ToolHtmlRenderer {
    ToolHtmlRenderer
}
