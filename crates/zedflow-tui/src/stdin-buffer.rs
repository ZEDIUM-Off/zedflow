use std::time::{Duration, Instant};
#[derive(Debug)]
pub struct StdinBuffer {
    buffer: String,
    timeout: Duration,
    started: Option<Instant>,
}
impl Default for StdinBuffer {
    fn default() -> Self {
        Self::new(10)
    }
}
impl StdinBuffer {
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            buffer: String::new(),
            timeout: Duration::from_millis(timeout_ms),
            started: None,
        }
    }
    pub fn process(&mut self, data: &str) -> Vec<String> {
        self.buffer.push_str(data);
        self.started.get_or_insert(Instant::now());
        self.take_complete(false)
    }
    pub fn flush(&mut self) -> Vec<String> {
        self.take_complete(true)
    }
    fn take_complete(&mut self, force: bool) -> Vec<String> {
        if !force
            && self.started.is_some_and(|t| t.elapsed() < self.timeout)
            && self.buffer == "\x1b"
        {
            return Vec::new();
        }
        let mut out = Vec::new();
        while !self.buffer.is_empty() {
            let end = if self.buffer.starts_with('\x1b') {
                self.buffer
                    .find(|c: char| c.is_ascii_alphabetic())
                    .map(|i| i + 1)
                    .unwrap_or(0)
            } else {
                self.buffer
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| i)
                    .unwrap_or(self.buffer.len())
            };
            if end == 0 {
                break;
            }
            out.push(self.buffer.drain(..end).collect());
        }
        if self.buffer.is_empty() {
            self.started = None;
        }
        out
    }
}
