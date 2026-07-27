use zedflow_tui::stdin_buffer::StdinBuffer;
#[test]
fn stdin_buffer_starts_empty() {
    let mut buffer = StdinBuffer::new(10);
    assert_eq!(buffer.process("a"), vec!["a"]);
}
