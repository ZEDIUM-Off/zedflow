use zedflow_tui::{Component, Tui};
struct Empty;
impl Component for Empty {
    fn render(&self, _: usize) -> Vec<String> {
        vec![]
    }
}
#[test]
fn empty_overlay_content_is_valid() {
    let mut tui = Tui::new();
    tui.show_overlay(Empty);
    assert!(tui.render(80).is_empty());
}
