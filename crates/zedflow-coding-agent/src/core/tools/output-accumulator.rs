//! Bounded-memory accumulation of streaming command output.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncatedBy, TruncationOptions, TruncationResult,
    truncate_tail,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputAccumulatorOptions {
    pub max_lines: usize,
    pub max_bytes: usize,
    pub temp_file_prefix: String,
}

impl Default for OutputAccumulatorOptions {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            max_bytes: DEFAULT_MAX_BYTES,
            temp_file_prefix: "pi-output".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OutputSnapshotOptions {
    pub persist_if_truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputSnapshot {
    pub content: String,
    pub truncation: TruncationResult,
    pub full_output_path: Option<PathBuf>,
}

pub struct OutputAccumulator {
    max_lines: usize,
    max_bytes: usize,
    max_rolling_bytes: usize,
    temp_file_prefix: String,
    raw_chunks: Vec<Vec<u8>>,
    decoder_pending: Vec<u8>,
    tail_text: String,
    tail_bytes: usize,
    tail_starts_at_line_boundary: bool,
    total_raw_bytes: usize,
    total_decoded_bytes: usize,
    completed_lines: usize,
    total_lines: usize,
    current_line_bytes: usize,
    has_open_line: bool,
    finished: bool,
    temp_file_path: Option<PathBuf>,
    temp_file: Option<File>,
}

impl OutputAccumulator {
    pub fn new(options: OutputAccumulatorOptions) -> Self {
        Self {
            max_lines: options.max_lines,
            max_bytes: options.max_bytes,
            max_rolling_bytes: options.max_bytes.saturating_mul(2).max(1),
            temp_file_prefix: options.temp_file_prefix,
            raw_chunks: Vec::new(),
            decoder_pending: Vec::new(),
            tail_text: String::new(),
            tail_bytes: 0,
            tail_starts_at_line_boundary: true,
            total_raw_bytes: 0,
            total_decoded_bytes: 0,
            completed_lines: 0,
            total_lines: 0,
            current_line_bytes: 0,
            has_open_line: false,
            finished: false,
            temp_file_path: None,
            temp_file: None,
        }
    }

    pub fn append(&mut self, data: impl AsRef<[u8]>) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::other(
                "cannot append to a finished output accumulator",
            ));
        }

        let data = data.as_ref();
        self.total_raw_bytes += data.len();
        self.decode(data, false);

        if self.temp_file.is_some() || self.should_use_temp_file() {
            self.ensure_temp_file()?;
            if let Some(file) = &mut self.temp_file {
                file.write_all(data)?;
            }
        } else if !data.is_empty() {
            self.raw_chunks.push(data.to_vec());
        }
        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        self.decode(&[], true);
        if self.should_use_temp_file() {
            self.ensure_temp_file()?;
        }
        Ok(())
    }

    pub fn snapshot(&mut self, options: OutputSnapshotOptions) -> io::Result<OutputSnapshot> {
        let mut truncation = truncate_tail(
            self.snapshot_text(),
            TruncationOptions {
                max_lines: self.max_lines,
                max_bytes: self.max_bytes,
            },
        );
        let truncated =
            self.total_lines > self.max_lines || self.total_decoded_bytes > self.max_bytes;
        let truncated_by = if truncated {
            truncation
                .truncated_by
                .or(Some(if self.total_decoded_bytes > self.max_bytes {
                    TruncatedBy::Bytes
                } else {
                    TruncatedBy::Lines
                }))
        } else {
            None
        };
        truncation.truncated = truncated;
        truncation.truncated_by = truncated_by;
        truncation.total_lines = self.total_lines;
        truncation.total_bytes = self.total_decoded_bytes;
        truncation.max_lines = self.max_lines;
        truncation.max_bytes = self.max_bytes;

        if options.persist_if_truncated && truncation.truncated {
            self.ensure_temp_file()?;
        }

        Ok(OutputSnapshot {
            content: truncation.content.clone(),
            truncation,
            full_output_path: self.temp_file_path.clone(),
        })
    }

    pub async fn close_temp_file(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.temp_file.take() {
            file.flush()?;
        }
        Ok(())
    }

    pub fn last_line_bytes(&self) -> usize {
        self.current_line_bytes
    }

    fn decode(&mut self, data: &[u8], finish: bool) {
        let mut bytes = std::mem::take(&mut self.decoder_pending);
        bytes.extend_from_slice(data);
        let mut decoded = String::new();
        let mut offset = 0;

        while offset < bytes.len() {
            match std::str::from_utf8(&bytes[offset..]) {
                Ok(text) => {
                    decoded.push_str(text);
                    offset = bytes.len();
                }
                Err(error) => {
                    let valid_end = offset + error.valid_up_to();
                    decoded.push_str(
                        std::str::from_utf8(&bytes[offset..valid_end])
                            .expect("validated UTF-8 prefix"),
                    );
                    match error.error_len() {
                        Some(length) => {
                            decoded.push('\u{fffd}');
                            offset = valid_end + length;
                        }
                        None if finish => {
                            decoded.push('\u{fffd}');
                            offset = bytes.len();
                        }
                        None => {
                            self.decoder_pending.extend_from_slice(&bytes[valid_end..]);
                            offset = bytes.len();
                        }
                    }
                }
            }
        }

        self.append_decoded_text(&decoded);
    }

    fn append_decoded_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let bytes = text.len();
        self.total_decoded_bytes += bytes;
        self.tail_text.push_str(text);
        self.tail_bytes += bytes;
        if self.tail_bytes > self.max_rolling_bytes.saturating_mul(2) {
            self.trim_tail();
        }

        let mut newlines = 0;
        let mut last_newline = None;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                newlines += 1;
                last_newline = Some(index);
            }
        }
        if let Some(last_newline) = last_newline {
            self.completed_lines += newlines;
            let tail = &text[last_newline + 1..];
            self.current_line_bytes = tail.len();
            self.has_open_line = !tail.is_empty();
        } else {
            self.current_line_bytes += bytes;
            self.has_open_line = true;
        }
        self.total_lines = self.completed_lines + usize::from(self.has_open_line);
    }

    fn trim_tail(&mut self) {
        if self.tail_text.len() <= self.max_rolling_bytes {
            self.tail_bytes = self.tail_text.len();
            return;
        }
        let mut start = self.tail_text.len() - self.max_rolling_bytes;
        while start < self.tail_text.len() && !self.tail_text.is_char_boundary(start) {
            start += 1;
        }
        if start != 0 {
            self.tail_starts_at_line_boundary = self.tail_text.as_bytes()[start - 1] == b'\n';
        }
        self.tail_text = self.tail_text[start..].to_owned();
        self.tail_bytes = self.tail_text.len();
    }

    fn snapshot_text(&self) -> &str {
        if self.tail_starts_at_line_boundary {
            return &self.tail_text;
        }
        self.tail_text
            .find('\n')
            .map_or(&self.tail_text, |index| &self.tail_text[index + 1..])
    }

    fn should_use_temp_file(&self) -> bool {
        self.total_raw_bytes > self.max_bytes
            || self.total_decoded_bytes > self.max_bytes
            || self.total_lines > self.max_lines
    }

    fn ensure_temp_file(&mut self) -> io::Result<()> {
        if self.temp_file_path.is_some() {
            return Ok(());
        }
        let (path, mut file) = create_temp_file(&self.temp_file_prefix)?;
        for chunk in self.raw_chunks.drain(..) {
            file.write_all(&chunk)?;
        }
        self.temp_file_path = Some(path);
        self.temp_file = Some(file);
        Ok(())
    }
}

impl Default for OutputAccumulator {
    fn default() -> Self {
        Self::new(OutputAccumulatorOptions::default())
    }
}

fn create_temp_file(prefix: &str) -> io::Result<(PathBuf, File)> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{epoch:x}-{id:x}.log"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}
