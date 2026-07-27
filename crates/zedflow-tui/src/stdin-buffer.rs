use std::time::{Duration, Instant};

const ESC: &str = "\x1b";
const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinEvent {
    Data(String),
    Paste(String),
}

#[derive(Debug)]
pub struct StdinBuffer {
    buffer: String,
    timeout: Duration,
    started: Option<Instant>,
    paste: Option<String>,
    pending_kitty_codepoint: Option<u32>,
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
            paste: None,
            pending_kitty_codepoint: None,
        }
    }

    pub fn process(&mut self, data: &str) -> Vec<StdinEvent> {
        self.process_string(data.to_owned())
    }

    pub fn process_bytes(&mut self, data: &[u8]) -> Vec<StdinEvent> {
        let data = if let [byte] = data
            && *byte > 127
        {
            format!("\x1b{}", char::from(*byte - 128))
        } else {
            String::from_utf8_lossy(data).into_owned()
        };
        self.process_string(data)
    }

    fn process_string(&mut self, data: String) -> Vec<StdinEvent> {
        if data.is_empty() && self.buffer.is_empty() && self.paste.is_none() {
            return vec![StdinEvent::Data(String::new())];
        }

        if let Some(paste) = &mut self.paste {
            paste.push_str(&data);
            return self.finish_paste();
        }

        self.buffer.push_str(&data);
        let mut events = Vec::new();

        if let Some(start) = self.buffer.find(PASTE_START) {
            let before = self.buffer[..start].to_owned();
            let (_, sequences) = extract_sequences(&before);
            self.emit_sequences(sequences, &mut events);
            self.pending_kitty_codepoint = None;
            self.paste = Some(self.buffer[start + PASTE_START.len()..].to_owned());
            self.buffer.clear();
            events.extend(self.finish_paste());
            return events;
        }

        let (remainder, sequences) = extract_sequences(&self.buffer);
        self.buffer = remainder;
        self.emit_sequences(sequences, &mut events);
        self.started = (!self.buffer.is_empty()).then(Instant::now);
        events
    }

    fn finish_paste(&mut self) -> Vec<StdinEvent> {
        let Some(paste) = &self.paste else {
            return Vec::new();
        };
        let Some(end) = paste.find(PASTE_END) else {
            return Vec::new();
        };
        let content = paste[..end].to_owned();
        let remaining = paste[end + PASTE_END.len()..].to_owned();
        self.paste = None;
        self.pending_kitty_codepoint = None;
        let mut events = vec![StdinEvent::Paste(content)];
        if !remaining.is_empty() {
            events.extend(self.process_string(remaining));
        }
        events
    }

    fn emit_sequences(&mut self, sequences: Vec<String>, events: &mut Vec<StdinEvent>) {
        for sequence in sequences {
            let raw = single_codepoint(&sequence);
            if raw.is_some() && raw == self.pending_kitty_codepoint {
                self.pending_kitty_codepoint = None;
                continue;
            }
            self.pending_kitty_codepoint = kitty_printable_codepoint(&sequence);
            events.push(StdinEvent::Data(sequence));
        }
    }

    pub fn flush_expired(&mut self) -> Vec<StdinEvent> {
        if self
            .started
            .is_some_and(|started| started.elapsed() >= self.timeout)
        {
            self.flush()
        } else {
            Vec::new()
        }
    }

    pub fn flush(&mut self) -> Vec<StdinEvent> {
        self.started = None;
        self.pending_kitty_codepoint = None;
        if self.buffer.is_empty() {
            Vec::new()
        } else {
            vec![StdinEvent::Data(std::mem::take(&mut self.buffer))]
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.started = None;
        self.paste = None;
        self.pending_kitty_codepoint = None;
    }

    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }
}

fn single_codepoint(value: &str) -> Option<u32> {
    let mut chars = value.chars();
    let value = chars.next()? as u32;
    chars.next().is_none().then_some(value)
}

fn kitty_printable_codepoint(sequence: &str) -> Option<u32> {
    let payload = sequence.strip_prefix("\x1b[")?.strip_suffix('u')?;
    if payload.contains(';') {
        return None;
    }
    let mut parts = payload.split(':');
    let codepoint = parts.next()?.parse().ok()?;
    if codepoint < 32
        || parts.clone().count() > 2
        || !parts.all(|part| part.is_empty() || part.parse::<u32>().is_ok())
    {
        return None;
    }
    Some(codepoint)
}

fn extract_sequences(buffer: &str) -> (String, Vec<String>) {
    let mut remaining = buffer;
    let mut sequences = Vec::new();
    while !remaining.is_empty() {
        if !remaining.starts_with(ESC) {
            let len = remaining.chars().next().unwrap().len_utf8();
            sequences.push(remaining[..len].to_owned());
            remaining = &remaining[len..];
            continue;
        }

        let Some(end) = escape_sequence_end(remaining) else {
            return (remaining.to_owned(), sequences);
        };
        if &remaining[..end] == "\x1b\x1b"
            && remaining
                .as_bytes()
                .get(end)
                .is_some_and(|byte| matches!(*byte, b'[' | b']' | b'O' | b'P' | b'_'))
        {
            sequences.push(ESC.to_owned());
            remaining = &remaining[1..];
        } else {
            sequences.push(remaining[..end].to_owned());
            remaining = &remaining[end..];
        }
    }
    (String::new(), sequences)
}

fn escape_sequence_end(data: &str) -> Option<usize> {
    let bytes = data.as_bytes();
    let second = *bytes.get(1)?;
    match second {
        b'[' => {
            if bytes.get(2) == Some(&b'M') {
                return (bytes.len() >= 6).then_some(6);
            }
            let index = bytes[2..]
                .iter()
                .position(|byte| (0x40..=0x7e).contains(byte))?
                + 2;
            let final_byte = bytes[index];
            let end = index + 1;
            if bytes.get(2) == Some(&b'<') && matches!(final_byte, b'M' | b'm') {
                let payload = &data[3..index];
                if payload.split(';').count() != 3
                    || !payload.split(';').all(|part| {
                        !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
                    })
                {
                    return None;
                }
            }
            Some(end)
        }
        b']' => string_terminator_end(bytes, true),
        b'P' | b'_' => string_terminator_end(bytes, false),
        b'O' => char_end(data, 2),
        _ => char_end(data, 1),
    }
}

fn char_end(data: &str, offset: usize) -> Option<usize> {
    let character = data.get(offset..)?.chars().next()?;
    Some(offset + character.len_utf8())
}

fn string_terminator_end(bytes: &[u8], allow_bel: bool) -> Option<usize> {
    for index in 2..bytes.len() {
        if allow_bel && bytes[index] == 7 {
            return Some(index + 1);
        }
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return Some(index + 2);
        }
    }
    None
}
