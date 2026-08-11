use std::thread;
use std::time::Duration;
use zedflow_tui::stdin_buffer::{StdinBuffer, StdinEvent};

fn data(value: &str) -> StdinEvent {
    StdinEvent::Data(value.to_owned())
}

#[test]
fn frames_text_and_terminal_sequences_across_chunks() {
    let mut buffer = StdinBuffer::new(10);
    assert_eq!(buffer.process("a世\x1b["), vec![data("a"), data("世")]);
    assert_eq!(buffer.get_buffer(), "\x1b[");
    assert_eq!(buffer.process("A\x1b]11;#fff"), vec![data("\x1b[A")]);
    assert_eq!(
        buffer.process("fff\x07\x1bP>|x\x1b\\\x1b_Gi=1;OK\x1b\\"),
        vec![
            data("\x1b]11;#ffffff\x07"),
            data("\x1bP>|x\x1b\\"),
            data("\x1b_Gi=1;OK\x1b\\"),
        ]
    );
}

#[test]
fn frames_mouse_ss3_meta_and_wezterm_escape() {
    let mut buffer = StdinBuffer::default();
    assert_eq!(
        buffer.process("\x1b[<35;20;5m\x1bOA\x1ba\x1b\x1b[27;1:3u"),
        vec![
            data("\x1b[<35;20;5m"),
            data("\x1bOA"),
            data("\x1ba"),
            data("\x1b"),
            data("\x1b[27;1:3u"),
        ]
    );
    assert!(buffer.process("\x1b[M a").is_empty());
    assert_eq!(buffer.process("b"), vec![data("\x1b[M ab")]);
}

#[test]
fn emits_bracketed_paste_separately_and_resumes_framing() {
    let mut buffer = StdinBuffer::default();
    assert_eq!(buffer.process("x\x1b[200~hello "), vec![data("x")]);
    assert_eq!(
        buffer.process("世界\x1b[201~y"),
        vec![StdinEvent::Paste("hello 世界".into()), data("y")]
    );
}

#[test]
fn converts_high_byte_and_suppresses_kitty_printable_duplicate() {
    let mut buffer = StdinBuffer::default();
    assert_eq!(buffer.process_bytes(&[0xe1]), vec![data("\x1ba")]);
    assert_eq!(buffer.process("\x1b[224uà"), vec![data("\x1b[224u")]);
    assert_eq!(
        buffer.process("\x1b[64;3u@"),
        vec![data("\x1b[64;3u"), data("@")]
    );
}

#[test]
fn flushes_incomplete_sequences_explicitly_or_after_timeout() {
    let mut buffer = StdinBuffer::new(1);
    assert!(buffer.process("\x1b[<35").is_empty());
    assert!(buffer.flush_expired().is_empty());
    thread::sleep(Duration::from_millis(2));
    assert_eq!(buffer.flush_expired(), vec![data("\x1b[<35")]);
    assert!(buffer.flush().is_empty());
}
