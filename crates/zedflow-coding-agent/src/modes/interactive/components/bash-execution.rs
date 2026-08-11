//! Streaming bash-command transcript rendering.

use zedflow_tui::{Component, Text};

use super::visual_truncate::truncate_to_visual_lines;
use crate::{
    truncate::{TruncationOptions, TruncationResult, truncate_tail},
    utils::ansi::strip_ansi,
};

const PREVIEW_LINES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    Complete,
    Cancelled,
    Error,
}

pub struct BashExecutionComponent {
    command: String,
    output_lines: Vec<String>,
    status: Status,
    exit_code: Option<i32>,
    truncation_result: Option<TruncationResult>,
    full_output_path: Option<String>,
    expanded: bool,
    _exclude_from_context: bool,
}

impl BashExecutionComponent {
    #[must_use]
    pub fn new(command: impl Into<String>, exclude_from_context: bool) -> Self {
        Self {
            command: command.into(),
            output_lines: Vec::new(),
            status: Status::Running,
            exit_code: None,
            truncation_result: None,
            full_output_path: None,
            expanded: false,
            _exclude_from_context: exclude_from_context,
        }
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn append_output(&mut self, chunk: &str) {
        let clean = strip_ansi(chunk).replace("\r\n", "\n").replace('\r', "\n");
        let mut new_lines = clean.split('\n');
        if let Some(first) = new_lines.next() {
            if let Some(last) = self.output_lines.last_mut() {
                last.push_str(first);
            } else {
                self.output_lines.push(first.to_owned());
            }
        }
        self.output_lines.extend(new_lines.map(str::to_owned));
    }

    pub fn set_complete(
        &mut self,
        exit_code: Option<i32>,
        cancelled: bool,
        truncation_result: Option<TruncationResult>,
        full_output_path: Option<String>,
    ) {
        self.exit_code = exit_code;
        self.status = if cancelled {
            Status::Cancelled
        } else if exit_code.is_some_and(|code| code != 0) {
            Status::Error
        } else {
            Status::Complete
        };
        self.truncation_result = truncation_result;
        self.full_output_path = full_output_path;
    }

    #[must_use]
    pub fn get_output(&self) -> String {
        self.output_lines.join("\n")
    }

    #[must_use]
    pub fn get_command(&self) -> &str {
        &self.command
    }
}

impl Component for BashExecutionComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let output = self.get_output();
        let context = truncate_tail(&output, TruncationOptions::default());
        let available: Vec<_> = if context.content.is_empty() {
            Vec::new()
        } else {
            context.content.split('\n').collect()
        };
        let hidden = available.len().saturating_sub(PREVIEW_LINES);
        let shown = if self.expanded {
            available.join("\n")
        } else {
            available[hidden..].join("\n")
        };
        let mut lines = vec![String::new(), "─".repeat(width.max(1))];
        lines.extend(Text::new(format!("$ {}", self.command), 1, 0).render(width));
        if !shown.is_empty() {
            if self.expanded {
                lines.extend(Text::new(format!("\n{shown}"), 1, 0).render(width));
            } else {
                lines.extend(
                    truncate_to_visual_lines(&format!("\n{shown}"), PREVIEW_LINES, width, 1)
                        .visual_lines,
                );
            }
        }
        let mut status = Vec::new();
        if self.status == Status::Running {
            status.push("Running... (escape to cancel)".to_owned());
        }
        if hidden > 0 {
            status.push(if self.expanded {
                "(ctrl+o to collapse)".into()
            } else {
                format!("... {hidden} more lines (ctrl+o to expand)")
            });
        }
        match self.status {
            Status::Cancelled => status.push("(cancelled)".into()),
            Status::Error => status.push(format!("(exit {})", self.exit_code.unwrap_or_default())),
            _ => {}
        }
        if (context.truncated
            || self
                .truncation_result
                .as_ref()
                .is_some_and(|result| result.truncated))
            && self.full_output_path.is_some()
        {
            status.push(format!(
                "Output truncated. Full output: {}",
                self.full_output_path.as_deref().unwrap()
            ));
        }
        if !status.is_empty() {
            lines.extend(Text::new(status.join("\n"), 1, 0).render(width));
        }
        lines.push("─".repeat(width.max(1)));
        lines
    }
}
