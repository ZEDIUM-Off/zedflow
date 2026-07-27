use zedflow_tui::{Component, Tui};
struct Wide;
impl Component for Wide {
    fn render(&self, _: usize) -> Vec<String> {
        vec!["wide".into()]
    }
}
#[test]
fn overlay_options_have_a_safe_render_path() {
    let mut tui = Tui::new();
    tui.show_overlay(Wide);
    assert_eq!(tui.overlay_count(), 1);
    assert_eq!(tui.render(1).len(), 1);
}
