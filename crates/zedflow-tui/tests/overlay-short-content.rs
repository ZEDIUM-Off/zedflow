use zedflow_tui::{Component, Tui};

struct Lines(&'static [&'static str]);
impl Component for Lines {
    fn render(&self, _: usize) -> Vec<String> {
        self.0.iter().map(|line| (*line).into()).collect()
    }
}

#[test]
fn centered_overlay_is_visible_when_base_is_shorter_than_viewport() {
    let mut tui = Tui::new();
    tui.root.add_child(Lines(&["one", "two", "three"]));
    tui.show_overlay(Lines(&["OVERLAY"]));

    let frame = tui.render_frame(40, 10);
    assert_eq!(frame.len(), 10);
    assert!(frame[4].contains("OVERLAY"));
}
