use crate::keys::set_kitty_protocol_active;
use crate::native_modifiers::{ModifierKey, is_native_modifier_pressed};
use crate::stdin_buffer::{StdinBuffer, StdinEvent};
use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PROGRESS_ACTIVE: &str = "\x1b]9;4;3\x07";
const PROGRESS_CLEAR: &str = "\x1b]9;4;0;\x07";
const KITTY_QUERY: &str = "\x1b[>7u\x1b[?u\x1b[c";
const APPLE_SHIFT_ENTER: &str = "\x1b[13;2u";
const NEGOTIATION_TIMEOUT: Duration = Duration::from_millis(150);

type Writer = Arc<Mutex<Box<dyn Write + Send>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardProtocolNegotiationSequence {
    KittyFlags(u32),
    DeviceAttributes,
}

pub fn parse_keyboard_protocol_negotiation_sequence(
    sequence: &str,
) -> Option<KeyboardProtocolNegotiationSequence> {
    if let Some(flags) = sequence
        .strip_prefix("\x1b[?")
        .and_then(|value| value.strip_suffix('u'))
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse().ok())
    {
        return Some(KeyboardProtocolNegotiationSequence::KittyFlags(flags));
    }
    let payload = sequence.strip_prefix("\x1b[?")?.strip_suffix('c')?;
    payload
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b';')
        .then_some(KeyboardProtocolNegotiationSequence::DeviceAttributes)
}

fn is_negotiation_prefix(sequence: &str) -> bool {
    sequence == "\x1b["
        || sequence.strip_prefix("\x1b[?").is_some_and(|tail| {
            tail.bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b';')
        })
}

pub fn is_apple_terminal_session() -> bool {
    cfg!(target_os = "macos") && env::var_os("TERM_PROGRAM").is_some_and(|v| v == "Apple_Terminal")
}

pub fn normalize_apple_terminal_input(data: &str, apple: bool, shift: bool) -> String {
    if apple && shift && data == "\r" {
        APPLE_SHIFT_ENTER.to_owned()
    } else {
        data.to_owned()
    }
}

pub trait Terminal {
    fn start(
        &mut self,
        on_input: Box<dyn FnMut(&str)>,
        on_resize: Box<dyn FnMut()>,
    ) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
    fn drain_input(&mut self, max_ms: u64, idle_ms: u64);
    fn write(&mut self, data: &str) -> io::Result<()>;
    fn columns(&self) -> u16;
    fn rows(&self) -> u16;
    fn kitty_protocol_active(&self) -> bool;
    fn move_by(&self, lines: i32) -> io::Result<()>;
    fn hide_cursor(&self) -> io::Result<()>;
    fn show_cursor(&self) -> io::Result<()>;
    fn clear_line(&self) -> io::Result<()>;
    fn clear_from_cursor(&self) -> io::Result<()>;
    fn clear_screen(&self) -> io::Result<()>;
    fn set_title(&self, title: &str) -> io::Result<()>;
    fn set_progress(&mut self, active: bool) -> io::Result<()>;
}

pub struct ProcessTerminal {
    writer: Writer,
    input_handler: Option<Box<dyn FnMut(&str)>>,
    resize_handler: Option<Box<dyn FnMut()>>,
    stdin_buffer: StdinBuffer,
    was_raw: bool,
    raw_changed: bool,
    started: bool,
    kitty_protocol_active: bool,
    modify_other_keys_active: bool,
    keyboard_protocol_pushed: bool,
    negotiation_buffer: String,
    negotiation_started: Option<Instant>,
    progress: Option<(mpsc::Sender<()>, JoinHandle<()>)>,
}

impl Default for ProcessTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessTerminal {
    pub fn new() -> Self {
        Self::with_writer(Box::new(io::stdout()))
    }

    pub fn with_writer(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            input_handler: None,
            resize_handler: None,
            stdin_buffer: StdinBuffer::default(),
            was_raw: false,
            raw_changed: false,
            started: false,
            kitty_protocol_active: false,
            modify_other_keys_active: false,
            keyboard_protocol_pushed: false,
            negotiation_buffer: String::new(),
            negotiation_started: None,
            progress: None,
        }
    }

    pub fn modify_other_keys_active(&self) -> bool {
        self.modify_other_keys_active
    }

    pub fn feed_input(&mut self, data: &str) {
        self.flush_expired_input();
        let events = self.stdin_buffer.process(data);
        self.handle_events(events);
    }

    pub fn feed_input_bytes(&mut self, data: &[u8]) {
        self.flush_expired_input();
        let events = self.stdin_buffer.process_bytes(data);
        self.handle_events(events);
    }

    pub fn flush_expired_input(&mut self) {
        let events = self.stdin_buffer.flush_expired();
        self.handle_events(events);
        if self
            .negotiation_started
            .is_some_and(|started| started.elapsed() >= NEGOTIATION_TIMEOUT)
        {
            self.flush_negotiation_as_input();
        }
    }

    pub fn flush_input(&mut self) {
        let events = self.stdin_buffer.flush();
        self.handle_events(events);
    }

    fn handle_events(&mut self, events: Vec<StdinEvent>) {
        for event in events {
            match event {
                StdinEvent::Paste(content) => {
                    self.forward_input(&format!("\x1b[200~{content}\x1b[201~"));
                }
                StdinEvent::Data(sequence) => self.handle_sequence(sequence),
            }
        }
    }

    fn handle_sequence(&mut self, sequence: String) {
        if !self.negotiation_buffer.is_empty() {
            let combined = format!("{}{sequence}", self.negotiation_buffer);
            if let Some(message) = parse_keyboard_protocol_negotiation_sequence(&combined) {
                self.clear_negotiation();
                self.handle_negotiation(message);
                return;
            }
            if is_negotiation_prefix(&combined) {
                self.negotiation_buffer = combined;
                self.negotiation_started = Some(Instant::now());
                return;
            }
            self.flush_negotiation_as_input();
        }

        if let Some(message) = parse_keyboard_protocol_negotiation_sequence(&sequence) {
            self.handle_negotiation(message);
        } else if is_negotiation_prefix(&sequence) {
            self.negotiation_buffer = sequence;
            self.negotiation_started = Some(Instant::now());
        } else {
            self.forward_input(&sequence);
        }
    }

    fn handle_negotiation(&mut self, message: KeyboardProtocolNegotiationSequence) {
        match message {
            KeyboardProtocolNegotiationSequence::KittyFlags(flags) if flags != 0 => {
                self.disable_modify_other_keys();
                self.kitty_protocol_active = true;
                set_kitty_protocol_active(true);
            }
            KeyboardProtocolNegotiationSequence::KittyFlags(_) => self.enable_modify_other_keys(),
            KeyboardProtocolNegotiationSequence::DeviceAttributes
                if !self.kitty_protocol_active =>
            {
                self.enable_modify_other_keys();
            }
            KeyboardProtocolNegotiationSequence::DeviceAttributes => {}
        }
    }

    fn forward_input(&mut self, sequence: &str) {
        let apple = sequence == "\r" && is_apple_terminal_session();
        let sequence = normalize_apple_terminal_input(
            sequence,
            apple,
            apple && is_native_modifier_pressed(ModifierKey::Shift),
        );
        if let Some(handler) = &mut self.input_handler {
            handler(&sequence);
        }
    }

    fn clear_negotiation(&mut self) {
        self.negotiation_buffer.clear();
        self.negotiation_started = None;
    }

    fn flush_negotiation_as_input(&mut self) {
        if !self.negotiation_buffer.is_empty() {
            let sequence = std::mem::take(&mut self.negotiation_buffer);
            self.negotiation_started = None;
            self.forward_input(&sequence);
        }
    }

    fn emit(&self, data: &str) -> io::Result<()> {
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        writer.write_all(data.as_bytes())?;
        writer.flush()
    }

    fn enable_modify_other_keys(&mut self) {
        if !self.kitty_protocol_active && !self.modify_other_keys_active {
            let _ = self.emit("\x1b[>4;2m");
            self.modify_other_keys_active = true;
        }
    }

    fn disable_modify_other_keys(&mut self) {
        if self.modify_other_keys_active {
            let _ = self.emit("\x1b[>4;0m");
            self.modify_other_keys_active = false;
        }
    }

    pub fn notify_resize(&mut self) {
        if let Some(handler) = &mut self.resize_handler {
            handler();
        }
    }

    pub fn drain_input(&mut self, max_ms: u64, idle_ms: u64) {
        self.disable_keyboard_protocols();
        let handler = self.input_handler.take();
        thread::sleep(Duration::from_millis(idle_ms.min(max_ms)));
        self.input_handler = handler;
    }

    pub fn move_by(&self, lines: i32) -> io::Result<()> {
        match lines.cmp(&0) {
            std::cmp::Ordering::Greater => self.emit(&format!("\x1b[{lines}B")),
            std::cmp::Ordering::Less => self.emit(&format!("\x1b[{}A", -lines)),
            std::cmp::Ordering::Equal => Ok(()),
        }
    }

    pub fn hide_cursor(&self) -> io::Result<()> {
        self.emit("\x1b[?25l")
    }
    pub fn show_cursor(&self) -> io::Result<()> {
        self.emit("\x1b[?25h")
    }
    pub fn clear_line(&self) -> io::Result<()> {
        self.emit("\x1b[K")
    }
    pub fn clear_from_cursor(&self) -> io::Result<()> {
        self.emit("\x1b[J")
    }
    pub fn clear_screen(&self) -> io::Result<()> {
        self.emit("\x1b[2J\x1b[H")
    }
    pub fn set_title(&self, title: &str) -> io::Result<()> {
        self.emit(&format!("\x1b]0;{title}\x07"))
    }

    pub fn set_progress(&mut self, active: bool) -> io::Result<()> {
        if active {
            self.emit(PROGRESS_ACTIVE)?;
            if self.progress.is_none() {
                let (sender, receiver) = mpsc::channel();
                let writer = Arc::clone(&self.writer);
                let handle = thread::spawn(move || {
                    while receiver.recv_timeout(Duration::from_secs(1)).is_err() {
                        let mut writer = writer.lock().unwrap_or_else(|error| error.into_inner());
                        let _ = writer.write_all(PROGRESS_ACTIVE.as_bytes());
                        let _ = writer.flush();
                    }
                });
                self.progress = Some((sender, handle));
            }
        } else {
            self.stop_progress();
            self.emit(PROGRESS_CLEAR)?;
        }
        Ok(())
    }

    fn stop_progress(&mut self) -> bool {
        let Some((sender, handle)) = self.progress.take() else {
            return false;
        };
        let _ = sender.send(());
        let _ = handle.join();
        true
    }

    fn disable_keyboard_protocols(&mut self) {
        self.clear_negotiation();
        if self.keyboard_protocol_pushed || self.kitty_protocol_active {
            let _ = self.emit("\x1b[<u");
            self.keyboard_protocol_pushed = false;
            self.kitty_protocol_active = false;
            set_kitty_protocol_active(false);
        }
        self.disable_modify_other_keys();
    }
}

impl Terminal for ProcessTerminal {
    fn start(
        &mut self,
        on_input: Box<dyn FnMut(&str)>,
        on_resize: Box<dyn FnMut()>,
    ) -> io::Result<()> {
        self.input_handler = Some(on_input);
        self.resize_handler = Some(on_resize);
        self.was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
        if io::stdin().is_terminal() && !self.was_raw {
            crossterm::terminal::enable_raw_mode()?;
            self.raw_changed = true;
        }
        self.emit("\x1b[?2004h")?;
        self.keyboard_protocol_pushed = true;
        self.clear_negotiation();
        self.emit(KITTY_QUERY)?;
        self.started = true;
        Ok(())
    }

    fn stop(&mut self) -> io::Result<()> {
        if !self.started {
            return Ok(());
        }
        if self.stop_progress() {
            self.emit(PROGRESS_CLEAR)?;
        }
        self.emit("\x1b[?2004l")?;
        self.disable_keyboard_protocols();
        self.stdin_buffer.clear();
        self.input_handler = None;
        self.resize_handler = None;
        if self.raw_changed {
            crossterm::terminal::disable_raw_mode()?;
            self.raw_changed = false;
        }
        self.started = false;
        Ok(())
    }

    fn drain_input(&mut self, max_ms: u64, idle_ms: u64) {
        ProcessTerminal::drain_input(self, max_ms, idle_ms);
    }

    fn write(&mut self, data: &str) -> io::Result<()> {
        self.emit(data)
    }

    fn columns(&self) -> u16 {
        crossterm::terminal::size()
            .ok()
            .map(|size| size.0)
            .filter(|size| *size != 0)
            .or_else(|| env::var("COLUMNS").ok()?.parse().ok())
            .unwrap_or(80)
    }

    fn rows(&self) -> u16 {
        crossterm::terminal::size()
            .ok()
            .map(|size| size.1)
            .filter(|size| *size != 0)
            .or_else(|| env::var("LINES").ok()?.parse().ok())
            .unwrap_or(24)
    }

    fn kitty_protocol_active(&self) -> bool {
        self.kitty_protocol_active
    }

    fn move_by(&self, lines: i32) -> io::Result<()> {
        ProcessTerminal::move_by(self, lines)
    }
    fn hide_cursor(&self) -> io::Result<()> {
        ProcessTerminal::hide_cursor(self)
    }
    fn show_cursor(&self) -> io::Result<()> {
        ProcessTerminal::show_cursor(self)
    }
    fn clear_line(&self) -> io::Result<()> {
        ProcessTerminal::clear_line(self)
    }
    fn clear_from_cursor(&self) -> io::Result<()> {
        ProcessTerminal::clear_from_cursor(self)
    }
    fn clear_screen(&self) -> io::Result<()> {
        ProcessTerminal::clear_screen(self)
    }
    fn set_title(&self, title: &str) -> io::Result<()> {
        ProcessTerminal::set_title(self, title)
    }
    fn set_progress(&mut self, active: bool) -> io::Result<()> {
        ProcessTerminal::set_progress(self, active)
    }
}

impl Drop for ProcessTerminal {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
