//! Rendered user messages in the interactive transcript.

use zedflow_tui::{Box as TuiBox, Component, Markdown};

const OSC133_ZONE_START: &str = "\x1b]133;A\x07";
const OSC133_ZONE_END: &str = "\x1b]133;B\x07\x1b]133;C\x07";

pub struct UserMessageComponent {
    text: String,
    output_pad: usize,
}

impl UserMessageComponent {
    #[must_use]
    pub fn new(text: impl Into<String>, output_pad: usize) -> Self {
        Self {
            text: text.into(),
            output_pad,
        }
    }

    pub fn set_output_pad(&mut self, padding: usize) {
        self.output_pad = padding;
    }
}

impl Component for UserMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut markdown = Markdown::new(&self.text);
        markdown.options_mut().preserve_ordered_list_markers = true;
        markdown.options_mut().preserve_backslash_escapes = true;
        let mut content = TuiBox::new(self.output_pad, 1);
        content.add_child(markdown);
        let mut lines = content.render(width);
        if let Some(first) = lines.first_mut() {
            first.insert_str(0, OSC133_ZONE_START);
        }
        if let Some(last) = lines.last_mut() {
            last.insert_str(0, OSC133_ZONE_END);
        }
        lines
    }
}
