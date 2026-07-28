use crate::keys::set_kitty_protocol_active;
use crate::native_modifiers::{ModifierKey, is_native_modifier_pressed};
use crate::stdin_buffer::{StdinBuffer, StdinEvent};
use std::collections::VecDeque;
use std::env;
use std::io::{self, IsTerminal, Read, Write};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PROGRESS_ACTIVE: &str = "\x1b]9;4;3\x07";
const PROGRESS_CLEAR: &str = "\x1b]9;4;0;\x07";
const KITTY_QUERY: &str = "\x1b[>7u\x1b[?u\x1b[c";
const APPLE_SHIFT_ENTER: &str = "\x1b[13;2u";
const NEGOTIATION_TIMEOUT: Duration = Duration::from_millis(150);

#[cfg(windows)]
#[allow(unsafe_code)]
fn enable_windows_virtual_terminal_input() {
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        ENABLE_VIRTUAL_TERMINAL_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE,
        SetConsoleMode,
    };

    // SAFETY: the process stdin handle and stack-owned mode pointer satisfy the Win32 contracts.
    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return;
        }
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_INPUT);
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Input(String),
    Resize,
}

pub trait Terminal {
    fn start(&mut self) -> io::Result<()>;
    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>>;
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
    reader: Option<Box<dyn Read + Send>>,
    native_events: Option<mpsc::Receiver<Vec<u8>>>,
    events: VecDeque<TerminalEvent>,
    stdin_buffer: StdinBuffer,
    last_size: Option<(u16, u16)>,
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
        Self::with_reader_and_writer(Box::new(io::stdin()), Box::new(io::stdout()))
    }

    pub fn with_writer(writer: Box<dyn Write + Send>) -> Self {
        Self::from_io(None, writer)
    }

    pub fn with_reader_and_writer(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
    ) -> Self {
        Self::from_io(Some(reader), writer)
    }

    fn from_io(reader: Option<Box<dyn Read + Send>>, writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
            reader,
            native_events: None,
            events: VecDeque::new(),
            stdin_buffer: StdinBuffer::default(),
            last_size: None,
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
        self.events
            .push_back(TerminalEvent::Input(normalize_apple_terminal_input(
                sequence,
                apple,
                apple && is_native_modifier_pressed(ModifierKey::Shift),
            )));
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
        self.events.push_back(TerminalEvent::Resize);
    }

    fn receive_native_input(&mut self, timeout: Duration) {
        let Some(receiver) = &self.native_events else {
            if !timeout.is_zero() {
                thread::sleep(timeout);
            }
            return;
        };
        match receiver.recv_timeout(timeout) {
            Ok(bytes) => self.feed_input_bytes(&bytes),
            Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {}
        }
    }

    fn detect_resize(&mut self) {
        let size = (self.columns(), self.rows());
        if self.last_size.replace(size).is_some_and(|old| old != size) {
            self.notify_resize();
        }
    }

    pub fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>> {
        self.flush_expired_input();
        self.detect_resize();
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }
        self.receive_native_input(timeout.min(Duration::from_millis(10)));
        self.flush_expired_input();
        self.detect_resize();
        Ok(self.events.pop_front())
    }

    pub fn drain_input(&mut self, max_ms: u64, idle_ms: u64) {
        self.disable_keyboard_protocols();
        let deadline = Instant::now() + Duration::from_millis(max_ms);
        while Instant::now() < deadline {
            let wait = Duration::from_millis(idle_ms)
                .min(deadline.saturating_duration_since(Instant::now()));
            let Some(receiver) = &self.native_events else {
                break;
            };
            match receiver.recv_timeout(wait) {
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
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
    fn start(&mut self) -> io::Result<()> {
        if self.started {
            return Ok(());
        }
        self.was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
        if io::stdin().is_terminal() && !self.was_raw {
            crossterm::terminal::enable_raw_mode()?;
            self.raw_changed = true;
        }
        #[cfg(windows)]
        enable_windows_virtual_terminal_input();
        self.emit("\x1b[?2004h")?;
        self.keyboard_protocol_pushed = true;
        self.clear_negotiation();
        self.emit(KITTY_QUERY)?;
        let (sender, receiver) = mpsc::channel();
        if let Some(mut reader) = self.reader.take() {
            thread::spawn(move || {
                let mut bytes = [0; 4096];
                while let Ok(count) = reader.read(&mut bytes) {
                    if count == 0 || sender.send(bytes[..count].to_vec()).is_err() {
                        break;
                    }
                }
            });
        }
        self.native_events = Some(receiver);
        self.last_size = Some((self.columns(), self.rows()));
        self.started = true;
        Ok(())
    }

    fn poll_event(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>> {
        ProcessTerminal::poll_event(self, timeout)
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
        self.events.clear();
        self.native_events = None;
        self.last_size = None;
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
