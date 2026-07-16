/// Default maximum output lines.
pub const DEFAULT_MAX_LINES: usize = 2000;
/// Default maximum output bytes.
pub const DEFAULT_MAX_BYTES: usize = 50 * 1024;
/// Maximum grep match line length.
pub const GREP_MAX_LINE_LENGTH: usize = 500;

/// Which truncation limit was hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncatedBy {
    /// Line limit was hit.
    Lines,
    /// Byte limit was hit.
    Bytes,
}

/// Result of truncating content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncationResult {
    /// Truncated content.
    pub content: String,
    /// Whether truncation occurred.
    pub truncated: bool,
    /// Which limit was hit.
    pub truncated_by: Option<TruncatedBy>,
    /// Total original lines.
    pub total_lines: usize,
    /// Total original bytes.
    pub total_bytes: usize,
    /// Output lines.
    pub output_lines: usize,
    /// Output bytes.
    pub output_bytes: usize,
    /// Whether the first retained line is partial.
    pub last_line_partial: bool,
    /// Whether the first line exceeded the byte limit.
    pub first_line_exceeds_limit: bool,
    /// Applied max lines.
    pub max_lines: usize,
    /// Applied max bytes.
    pub max_bytes: usize,
}

/// Truncation options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruncationOptions {
    /// Maximum lines.
    pub max_lines: usize,
    /// Maximum bytes.
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

/// Single-line truncation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruncateLineResult {
    /// Result text.
    pub text: String,
    /// Whether truncation occurred.
    pub was_truncated: bool,
}

/// Format bytes as a human-readable size.
#[must_use]
pub fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate content from the head, keeping complete lines only.
#[must_use]
pub fn truncate_head(content: &str, options: Option<TruncationOptions>) -> TruncationResult {
    let options = options.unwrap_or_default();
    let total_bytes = content.len();
    let lines: Vec<&str> = content.split('\n').collect();
    let total_lines = lines.len();

    if total_lines <= options.max_lines && total_bytes <= options.max_bytes {
        return no_truncation(content, total_lines, total_bytes, options);
    }

    if lines.first().map_or(0, |line| line.len()) > options.max_bytes {
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

    let mut output_lines = Vec::new();
    let mut output_bytes = 0;
    let mut truncated_by = TruncatedBy::Lines;

    for (index, line) in lines.iter().enumerate().take(options.max_lines) {
        let line_bytes = line.len() + usize::from(index > 0);
        if output_bytes + line_bytes > options.max_bytes {
            truncated_by = TruncatedBy::Bytes;
            break;
        }
        output_lines.push(*line);
        output_bytes += line_bytes;
    }

    if output_lines.len() >= options.max_lines && output_bytes <= options.max_bytes {
        truncated_by = TruncatedBy::Lines;
    }

    let output = output_lines.join("\n");
    let final_bytes = output.len();
    TruncationResult {
        content: output,
        truncated: true,
        truncated_by: Some(truncated_by),
        total_lines,
        total_bytes,
        output_lines: output_lines.len(),
        output_bytes: final_bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines: options.max_lines,
        max_bytes: options.max_bytes,
    }
}

/// Truncate content from the tail.
#[must_use]
pub fn truncate_tail(content: &str, options: Option<TruncationOptions>) -> TruncationResult {
    let options = options.unwrap_or_default();
    let total_bytes = content.len();
    let mut lines: Vec<&str> = content.split('\n').collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }
    let total_lines = lines.len();

    if total_lines <= options.max_lines && total_bytes <= options.max_bytes {
        return no_truncation(content, total_lines, total_bytes, options);
    }

    let mut output_lines = Vec::new();
    let mut output_bytes = 0;
    let mut truncated_by = TruncatedBy::Lines;
    let mut last_line_partial = false;

    for line in lines.iter().rev().take(options.max_lines) {
        let line_bytes = line.len() + usize::from(!output_lines.is_empty());
        if output_bytes + line_bytes > options.max_bytes {
            truncated_by = TruncatedBy::Bytes;
            if output_lines.is_empty() {
                let truncated_line = truncate_string_to_bytes_from_end(line, options.max_bytes);
                output_bytes = truncated_line.len();
                output_lines.insert(0, truncated_line);
                last_line_partial = true;
            }
            break;
        }
        output_lines.insert(0, (*line).to_string());
        output_bytes += line_bytes;
    }

    if output_lines.len() >= options.max_lines && output_bytes <= options.max_bytes {
        truncated_by = TruncatedBy::Lines;
    }

    let output = output_lines.join("\n");
    let final_bytes = output.len();
    TruncationResult {
        content: output,
        truncated: true,
        truncated_by: Some(truncated_by),
        total_lines,
        total_bytes,
        output_lines: output_lines.len(),
        output_bytes: final_bytes,
        last_line_partial,
        first_line_exceeds_limit: false,
        max_lines: options.max_lines,
        max_bytes: options.max_bytes,
    }
}

/// Truncate one line to a character limit with Pi's suffix.
#[must_use]
pub fn truncate_line(line: &str, max_chars: Option<usize>) -> TruncateLineResult {
    let max_chars = max_chars.unwrap_or(GREP_MAX_LINE_LENGTH);
    if line.chars().count() <= max_chars {
        return TruncateLineResult {
            text: line.to_string(),
            was_truncated: false,
        };
    }
    TruncateLineResult {
        text: format!(
            "{}... [truncated]",
            line.chars().take(max_chars).collect::<String>()
        ),
        was_truncated: true,
    }
}

fn no_truncation(
    content: &str,
    total_lines: usize,
    total_bytes: usize,
    options: TruncationOptions,
) -> TruncationResult {
    TruncationResult {
        content: content.to_string(),
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

fn truncate_string_to_bytes_from_end(value: &str, max_bytes: usize) -> String {
    if max_bytes == 0 {
        return String::new();
    }
    for (index, _) in value.char_indices().rev() {
        let tail = &value[index..];
        if tail.len() > max_bytes {
            let next_index = value[index..]
                .char_indices()
                .nth(1)
                .map_or(value.len(), |(offset, _)| index + offset);
            return value[next_index..].to_string();
        }
    }
    value.to_string()
}
