//! Width-aware transcript separator.

use std::sync::Arc;
use zedflow_tui::Component;

type Color = Arc<dyn Fn(&str) -> String + Send + Sync>;

#[derive(Clone)]
pub struct DynamicBorder {
    color: Color,
}

impl DynamicBorder {
    #[must_use]
    pub fn new(color: impl Fn(&str) -> String + Send + Sync + 'static) -> Self {
        Self {
            color: Arc::new(color),
        }
    }
}

impl Default for DynamicBorder {
    fn default() -> Self {
        Self::new(str::to_owned)
    }
}

impl Component for DynamicBorder {
    fn render(&self, width: usize) -> Vec<String> {
        vec![(self.color)(&"─".repeat(width.max(1)))]
    }
}
