use std::{cell::RefCell, rc::Rc};
use zedflow_tui::{CURSOR_MARKER, Component, Input};

#[test]
fn edits_graphemes_words_and_lines() {
    let mut input = Input::new("a👨‍👩‍👧‍👦");
    input.handle_input("\x7f");
    assert_eq!(input.value(), "a");

    input.set_value("foo.bar");
    input.handle_input("\x05");
    input.handle_input("\x17");
    assert_eq!(input.value(), "foo.");
    input.handle_input("\x15");
    assert_eq!(input.value(), "");
    input.handle_input("\x19");
    assert_eq!(input.value(), "foo.bar");
}

#[test]
fn paste_is_cleaned_and_undo_restores_snapshot() {
    let mut input = Input::new("ab");
    input.handle_input("\x1b[D");
    input.handle_input("\x1b[200~x\r\ny\tz\x1b[201~");
    assert_eq!(input.value(), "axy    zb");
    input.handle_input("\x1f");
    assert_eq!(input.value(), "ab");
    assert_eq!(input.cursor(), 1);
}

#[test]
fn decodes_kitty_printable_and_coalesces_typed_undo() {
    let mut input = Input::default();
    input.handle_input("\x1b[97u");
    input.handle_input("b");
    input.handle_input("\x1f");
    assert_eq!(input.value(), "");
}

#[test]
fn invokes_submit_and_escape_callbacks() {
    let submitted = Rc::new(RefCell::new(String::new()));
    let escaped = Rc::new(RefCell::new(false));
    let mut input = Input::new("hello");
    let out = Rc::clone(&submitted);
    input.on_submit = Some(Box::new(move |value| *out.borrow_mut() = value.into()));
    let out = Rc::clone(&escaped);
    input.on_escape = Some(Box::new(move || *out.borrow_mut() = true));
    input.handle_input("\r");
    input.handle_input("\x1b");
    assert_eq!(&*submitted.borrow(), "hello");
    assert!(*escaped.borrow());
}

#[test]
fn renders_cursor_marker_and_horizontal_viewport() {
    let mut input = Input::new("0123456789");
    input.set_focused(true);
    let line = input.render(8).remove(0);
    assert!(line.contains(CURSOR_MARKER));
    assert_eq!(zedflow_tui::visible_width(&line), 8);
    assert!(!line.contains("0123"));
}
