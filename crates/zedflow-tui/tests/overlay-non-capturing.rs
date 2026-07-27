use zedflow_tui::{Component, Tui};
struct Line;
impl Component for Line {
    fn render(&self, _: usize) -> Vec<String> {
        vec!["overlay".into()]
    }
}
#[test]
fn overlays_render_after_root_content() {
    let mut tui = Tui::new();
    tui.root.add_child(Line);
    tui.show_overlay(Line);
    assert_eq!(tui.render(20), vec!["overlay", "overlay"]);
}
