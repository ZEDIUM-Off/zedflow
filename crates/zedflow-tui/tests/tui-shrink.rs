use zedflow_tui::{Component, Tui};

struct Lines(Vec<String>);
impl Component for Lines {
    fn render(&self, _width: usize) -> Vec<String> {
        self.0.clone()
    }
}

#[test]
fn clear_removes_all_rendered_content() {
    let mut tui = Tui::new();
    tui.root
        .add_child(Lines(vec!["first".into(), "second".into()]));
    assert_eq!(tui.render(20).len(), 2);
    tui.root.clear();
    assert!(tui.render(20).is_empty());
}
