//! Visual-line truncation shared by transcript renderers.

use zedflow_tui::{Component, Text};

/// Result of retaining the final visual lines of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualTruncateResult {
    pub visual_lines: Vec<String>,
    pub skipped_count: usize,
}

/// Render `text` at `width` and retain at most its final `max_visual_lines`.
#[must_use]
pub fn truncate_to_visual_lines(
    text: &str,
    max_visual_lines: usize,
    width: usize,
    padding_x: usize,
) -> VisualTruncateResult {
    let lines = Text::new(text, padding_x, 0).render(width);
    let skipped_count = lines.len().saturating_sub(max_visual_lines);
    VisualTruncateResult {
        visual_lines: lines.into_iter().skip(skipped_count).collect(),
        skipped_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_the_tail_of_wrapped_output() {
        let result = truncate_to_visual_lines("one two three", 2, 5, 0);
        assert_eq!(result.skipped_count, 1);
        assert_eq!(result.visual_lines, ["wo th", "ree  "]);
    }
}
