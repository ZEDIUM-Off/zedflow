use std::cell::RefCell;
use std::rc::Rc;
use zedflow_tui::{Component, Tui};

struct Recorder(Rc<RefCell<Vec<String>>>);
impl Component for Recorder {
    fn render(&self, _width: usize) -> Vec<String> {
        vec![String::new()]
    }
    fn handle_input(&mut self, data: &str) {
        self.0.borrow_mut().push(data.into());
    }
}

#[test]
fn forwards_input_to_focused_overlay() {
    let inputs = Rc::new(RefCell::new(Vec::new()));
    let mut tui = Tui::new();
    tui.root.add_child(Recorder(inputs.clone()));
    tui.dispatch_input("q");
    assert_eq!(&*inputs.borrow(), &["q"]);
}
