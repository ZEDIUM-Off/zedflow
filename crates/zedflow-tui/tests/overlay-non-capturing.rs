use std::{cell::Cell, rc::Rc};
use zedflow_tui::{Component, OverlayOptions, Tui};

struct Focus(Rc<Cell<bool>>);
impl Component for Focus {
    fn render(&self, _: usize) -> Vec<String> {
        vec!["overlay".into()]
    }
    fn set_focused(&mut self, focused: bool) {
        self.0.set(focused);
    }
    fn is_focused(&self) -> bool {
        self.0.get()
    }
}

#[test]
fn non_capturing_overlay_only_takes_focus_when_requested() {
    let state = Rc::new(Cell::new(false));
    let mut tui = Tui::new();
    let id = tui.show_overlay_with_options(
        Focus(state.clone()),
        OverlayOptions {
            non_capturing: true,
            ..Default::default()
        },
    );

    assert!(!state.get());
    assert!(!tui.is_overlay_focused(id));
    assert!(tui.focus_overlay(id));
    assert!(state.get());
    assert!(tui.unfocus_overlay(id));
    assert!(!state.get());
}
