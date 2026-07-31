//! Width-aware transcript separator.

use zedflow_tui::Component;

/// Pi's border line, recalculated for the active viewport width.
#[derive(Debug, Clone, Default)]
pub struct DynamicBorder;

impl Component for DynamicBorder {
    fn render(&self, width: usize) -> Vec<String> {
        vec!["─".repeat(width.max(1))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_never_disappears_in_a_zero_width_layout() {
        assert_eq!(DynamicBorder.render(0), ["─"]);
    }
}
