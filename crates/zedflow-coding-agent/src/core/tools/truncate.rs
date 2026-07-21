//! Shared truncation utilities for tool output.

pub const DEFAULT_MAX_LINES: usize = 2_000;
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024;
pub const GREP_MAX_LINE_LENGTH: usize = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TruncatedBy {
    Lines,
    Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TruncationResult {
    pub content: String,
    pub truncated: bool,
    pub truncated_by: Option<TruncatedBy>,
    pub total_lines: usize,
    pub total_bytes: usize,
    pub output_lines: usize,
    pub output_bytes: usize,
    pub last_line_partial: bool,
    pub first_line_exceeds_limit: bool,
    pub max_lines: usize,
    pub max_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TruncationOptions {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl Default for TruncationOptions {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn lines_for_counting(content: &str) -> Vec<&str> {
    if content.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<_> = content.split('\n').collect();
    if content.ends_with('\n') {
        lines.pop();
    }
    lines
}

pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn truncate_head(content: &str, options: TruncationOptions) -> TruncationResult {
    let total_bytes = content.len();
    let lines = lines_for_counting(content);
    let total_lines = lines.len();

    if total_lines <= options.max_lines && total_bytes <= options.max_bytes {
        return unchanged(content, total_lines, total_bytes, options);
    }

    if lines
        .first()
        .is_some_and(|line| line.len() > options.max_bytes)
    {
        return TruncationResult {
            content: String::new(),
            truncated: true,
            truncated_by: Some(TruncatedBy::Bytes),
            total_lines,
            total_bytes,
            output_lines: 0,
            output_bytes: 0,
            last_line_partial: false,
            first_line_exceeds_limit: true,
            max_lines: options.max_lines,
            max_bytes: options.max_bytes,
        };
    }

    let mut output = Vec::new();
    let mut output_bytes = 0;
    let mut truncated_by = TruncatedBy::Lines;

    for (index, line) in lines.iter().take(options.max_lines).enumerate() {
        let line_bytes = line.len() + usize::from(index > 0);
        if output_bytes + line_bytes > options.max_bytes {
            truncated_by = TruncatedBy::Bytes;
            break;
        }
        output.push(*line);
        output_bytes += line_bytes;
    }
    if output.len() >= options.max_lines && output_bytes <= options.max_bytes {
        truncated_by = TruncatedBy::Lines;
    }

    truncated(
        content,
        &output.join("\n"),
        total_lines,
        output.len(),
        truncated_by,
        options,
        false,
    )
}

pub fn truncate_tail(content: &str, options: TruncationOptions) -> TruncationResult {
    let total_bytes = content.len();
    let lines = lines_for_counting(content);
    let total_lines = lines.len();

    if total_lines <= options.max_lines && total_bytes <= options.max_bytes {
        return unchanged(content, total_lines, total_bytes, options);
    }

    let mut output = Vec::new();
    let mut output_bytes = 0;
    let mut truncated_by = TruncatedBy::Lines;
    let mut last_line_partial = false;

    for line in lines.iter().rev().take(options.max_lines) {
        let line_bytes = line.len() + usize::from(!output.is_empty());
        if output_bytes + line_bytes > options.max_bytes {
            truncated_by = TruncatedBy::Bytes;
            if output.is_empty() {
                let partial = truncate_from_end(line, options.max_bytes);
                output_bytes = partial.len();
                output.push(partial);
                last_line_partial = true;
            }
            break;
        }
        output.push(*line);
        output_bytes += line_bytes;
    }
    if output.len() >= options.max_lines && output_bytes <= options.max_bytes {
        truncated_by = TruncatedBy::Lines;
    }
    output.reverse();

    truncated(
        content,
        &output.join("\n"),
        total_lines,
        output.len(),
        truncated_by,
        options,
        last_line_partial,
    )
}

fn unchanged(
    content: &str,
    total_lines: usize,
    total_bytes: usize,
    options: TruncationOptions,
) -> TruncationResult {
    TruncationResult {
        content: content.to_owned(),
        truncated: false,
        truncated_by: None,
        total_lines,
        total_bytes,
        output_lines: total_lines,
        output_bytes: total_bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines: options.max_lines,
        max_bytes: options.max_bytes,
    }
}

fn truncated(
    original: &str,
    content: &str,
    total_lines: usize,
    output_lines: usize,
    truncated_by: TruncatedBy,
    options: TruncationOptions,
    last_line_partial: bool,
) -> TruncationResult {
    TruncationResult {
        content: content.to_owned(),
        truncated: true,
        truncated_by: Some(truncated_by),
        total_lines,
        total_bytes: original.len(),
        output_lines,
        output_bytes: content.len(),
        last_line_partial,
        first_line_exceeds_limit: false,
        max_lines: options.max_lines,
        max_bytes: options.max_bytes,
    }
}

fn truncate_from_end(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut start = value.len() - max_bytes;
    while start < value.len() && !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}

pub fn truncate_line(line: &str, max_chars: usize) -> (String, bool) {
    let utf16: Vec<_> = line.encode_utf16().collect();
    if utf16.len() <= max_chars {
        return (line.to_owned(), false);
    }
    (
        format!(
            "{}... [truncated]",
            String::from_utf16_lossy(&utf16[..max_chars])
        ),
        true,
    )
}
