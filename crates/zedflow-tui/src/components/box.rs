use std::sync::Arc;

use crate::{Component, utils::apply_background_to_line};

type Background = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Container that applies padding and an optional background to its children.
pub struct Box {
    pub children: Vec<std::boxed::Box<dyn Component>>,
    pub padding_x: usize,
    pub padding_y: usize,
    background: Option<Background>,
}

impl Box {
    pub fn new(padding_x: usize, padding_y: usize) -> Self {
        Self {
            children: Vec::new(),
            padding_x,
            padding_y,
            background: None,
        }
    }

    pub fn add_child(&mut self, child: impl Component + 'static) {
        self.children.push(std::boxed::Box::new(child));
    }

    pub fn remove_child(&mut self, index: usize) -> Option<std::boxed::Box<dyn Component>> {
        (index < self.children.len()).then(|| self.children.remove(index))
    }

    pub fn clear(&mut self) {
        self.children.clear();
    }

    pub fn set_background(&mut self, background: Option<Background>) {
        self.background = background;
    }

    fn padded_line(&self, line: &str, width: usize) -> String {
        if let Some(background) = &self.background {
            apply_background_to_line(line, width, |text| background(text))
        } else {
            let padding = width.saturating_sub(crate::utils::visible_width(line));
            format!("{line}{}", " ".repeat(padding))
        }
    }
}

impl Component for Box {
    fn render(&self, width: usize) -> Vec<String> {
        if self.children.is_empty() {
            return Vec::new();
        }
        let content_width = width.saturating_sub(self.padding_x * 2).max(1);
        let left = " ".repeat(self.padding_x);
        let child_lines: Vec<String> = self
            .children
            .iter()
            .flat_map(|child| child.render(content_width))
            .map(|line| format!("{left}{line}"))
            .collect();
        if child_lines.is_empty() {
            return Vec::new();
        }

        let mut lines = Vec::with_capacity(child_lines.len() + self.padding_y * 2);
        lines.extend((0..self.padding_y).map(|_| self.padded_line("", width)));
        lines.extend(child_lines.iter().map(|line| self.padded_line(line, width)));
        lines.extend((0..self.padding_y).map(|_| self.padded_line("", width)));
        lines
    }

    fn invalidate(&mut self) {
        for child in &mut self.children {
            child.invalidate();
        }
    }
}
