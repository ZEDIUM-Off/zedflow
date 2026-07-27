use zedflow_tui::utils::slice_with_width;
use zedflow_tui::{KillRing, UndoStack};

#[test]
fn editor_state_helpers_match_pi() {
    assert_eq!(slice_with_width("abcdef", 2, 3, false), ("cde".into(), 3));
    let mut ring = KillRing::new();
    ring.push("world", false, false);
    ring.push("hello ", true, true);
    assert_eq!(ring.peek(), Some("hello world"));
    let mut undo = UndoStack::new();
    undo.push(&vec!["before"]);
    assert_eq!(undo.pop(), Some(vec!["before"]));
}
