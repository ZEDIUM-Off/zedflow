use zedflow_tui::utils::slice_with_width;
use zedflow_tui::{Component, Editor, KillRing, UndoStack};

#[test]
fn editor_state_helpers_match_pi() {
    assert_eq!(slice_with_width("abcdef", 2, 3, false), ("cde".into(), 3));
    let mut ring = KillRing::default();
    ring.push("world", false, false);
    ring.push("hello ", true, true);
    assert_eq!(ring.peek(), Some("hello world"));
    let mut undo = UndoStack::default();
    undo.push(&vec!["before"]);
    assert_eq!(undo.pop(), Some(vec!["before"]));
}

#[test]
fn editor_ports_multiline_unicode_history_undo_and_paste() {
    let mut editor = Editor::new();
    editor.set_text("one\r\ntwo\t😀");
    assert_eq!(editor.get_text(), "one\ntwo    😀");
    assert_eq!(editor.get_cursor(), (1, 11));

    editor.handle_input("\x7f");
    assert_eq!(editor.get_text(), "one\ntwo    ");
    editor.handle_input("\x1f");
    assert_eq!(editor.get_text(), "one\ntwo    😀");

    editor.add_to_history("old");
    editor.set_text("");
    editor.handle_input("\x1b[A");
    assert_eq!(editor.get_text(), "old");

    let large = "line\n".repeat(11);
    editor.set_text("");
    editor.handle_input(&format!("\x1b[200~{large}\x1b[201~"));
    assert!(editor.get_text().starts_with("[paste #1 +"));
    assert_eq!(editor.get_expanded_text(), large);
}

#[test]
fn word_wrap_preserves_every_byte_and_width() {
    let text = "Lorem ipsum dolor sit amet,    consectetur你";
    let chunks = Editor::word_wrap_line(text, 12);
    assert_eq!(
        chunks.iter().map(|c| c.text.as_str()).collect::<String>(),
        text
    );
    assert!(
        chunks
            .iter()
            .all(|c| zedflow_tui::visible_width(&c.text) <= 12)
    );
}
