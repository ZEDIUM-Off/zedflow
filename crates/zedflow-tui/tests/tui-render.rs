use zedflow_tui::{Component, Tui};

struct Lines(Vec<String>);
impl Component for Lines {
    fn render(&self, _width: usize) -> Vec<String> {
        self.0.clone()
    }
}

#[test]
fn renders_root_then_top_overlay() {
    let mut tui = Tui::new();
    tui.root.add_child(Lines(vec!["base".into()]));
    tui.show_overlay(Lines(vec!["overlay".into()]));
    assert_eq!(tui.render(20), vec!["base", "overlay"]);
}

#[test]
fn removing_overlay_restores_root() {
    let mut tui = Tui::new();
    let id = tui.show_overlay(Lines(vec!["overlay".into()]));
    assert_eq!(tui.overlay_count(), 1);
    assert!(tui.hide_overlay(id).is_some());
    assert_eq!(tui.overlay_count(), 0);
    assert!(tui.hide_overlay(0).is_none());
}
