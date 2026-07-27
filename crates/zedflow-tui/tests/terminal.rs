use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use zedflow_tui::terminal::{
    KeyboardProtocolNegotiationSequence, ProcessTerminal, Terminal, normalize_apple_terminal_input,
    parse_keyboard_protocol_negotiation_sequence,
};

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn harness() -> (ProcessTerminal, Arc<Mutex<Vec<u8>>>) {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    (
        ProcessTerminal::with_writer(Box::new(SharedWriter(bytes.clone()))),
        bytes,
    )
}

fn output(bytes: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(bytes.lock().unwrap().clone()).unwrap()
}

#[test]
fn normalizes_only_apple_shift_enter() {
    assert_eq!(
        normalize_apple_terminal_input("\r", true, true),
        "\x1b[13;2u"
    );
    assert_eq!(normalize_apple_terminal_input("\r", true, false), "\r");
    assert_eq!(
        normalize_apple_terminal_input("\x1b[3~", true, true),
        "\x1b[3~"
    );
}

#[test]
fn parses_keyboard_protocol_negotiation_strictly() {
    assert_eq!(
        parse_keyboard_protocol_negotiation_sequence("\x1b[?7u"),
        Some(KeyboardProtocolNegotiationSequence::KittyFlags(7))
    );
    assert_eq!(
        parse_keyboard_protocol_negotiation_sequence("\x1b[?62;4;52c"),
        Some(KeyboardProtocolNegotiationSequence::DeviceAttributes)
    );
    assert_eq!(
        parse_keyboard_protocol_negotiation_sequence("x\x1b[?7u"),
        None
    );
}

#[test]
fn lifecycle_negotiates_protocols_and_restores_terminal_modes() {
    let (mut terminal, bytes) = harness();
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let recorded = inputs.clone();
    terminal
        .start(
            Box::new(move |data| recorded.lock().unwrap().push(data.to_owned())),
            Box::new(|| {}),
        )
        .unwrap();
    assert!(output(&bytes).starts_with("\x1b[?2004h\x1b[>7u\x1b[?u\x1b[c"));

    terminal.feed_input("a\x1b[?0u");
    assert_eq!(&*inputs.lock().unwrap(), &["a"]);
    assert!(terminal.modify_other_keys_active());

    terminal.feed_input("\x1b[?7u");
    assert!(terminal.kitty_protocol_active());
    assert!(!terminal.modify_other_keys_active());

    terminal.stop().unwrap();
    let output = output(&bytes);
    assert!(output.ends_with("\x1b[?2004l\x1b[<u"));
}

#[test]
fn split_negotiation_and_paste_are_not_forwarded_as_plain_fragments() {
    let (mut terminal, _) = harness();
    let inputs = Arc::new(Mutex::new(Vec::new()));
    let recorded = inputs.clone();
    terminal
        .start(
            Box::new(move |data| recorded.lock().unwrap().push(data.to_owned())),
            Box::new(|| {}),
        )
        .unwrap();
    terminal.feed_input("\x1b[?7");
    terminal.feed_input("u\x1b[200~hello");
    terminal.feed_input(" world\x1b[201~");
    assert!(terminal.kitty_protocol_active());
    assert_eq!(&*inputs.lock().unwrap(), &["\x1b[200~hello world\x1b[201~"]);
    terminal.stop().unwrap();
}

#[test]
fn terminal_control_methods_emit_pi_sequences() {
    let (terminal, bytes) = harness();
    terminal.move_by(-2).unwrap();
    terminal.hide_cursor().unwrap();
    terminal.clear_line().unwrap();
    terminal.clear_from_cursor().unwrap();
    terminal.clear_screen().unwrap();
    terminal.set_title("pi").unwrap();
    assert_eq!(
        output(&bytes),
        "\x1b[2A\x1b[?25l\x1b[K\x1b[J\x1b[2J\x1b[H\x1b]0;pi\x07"
    );
}
