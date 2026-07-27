use std::{
    cell::RefCell,
    io,
    rc::Rc,
    sync::{Arc, Mutex},
};
use zedflow_tui::{CURSOR_MARKER, Component, Tui, terminal::Terminal};

struct Lines(Rc<RefCell<Vec<String>>>);
impl Component for Lines {
    fn render(&self, _: usize) -> Vec<String> {
        self.0.borrow().clone()
    }
}

struct TestTerminal {
    writes: Arc<Mutex<Vec<String>>>,
    width: u16,
    height: u16,
}
impl Terminal for TestTerminal {
    fn start(&mut self, _: Box<dyn FnMut(&str)>, _: Box<dyn FnMut()>) -> io::Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn drain_input(&mut self, _: u64, _: u64) {}
    fn write(&mut self, data: &str) -> io::Result<()> {
        self.writes.lock().unwrap().push(data.into());
        Ok(())
    }
    fn columns(&self) -> u16 {
        self.width
    }
    fn rows(&self) -> u16 {
        self.height
    }
    fn kitty_protocol_active(&self) -> bool {
        false
    }
    fn move_by(&self, _: i32) -> io::Result<()> {
        Ok(())
    }
    fn hide_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn show_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_line(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_from_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_screen(&self) -> io::Result<()> {
        Ok(())
    }
    fn set_title(&self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn set_progress(&mut self, _: bool) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn lifecycle_performs_first_and_differential_renders() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let terminal = TestTerminal {
        writes: writes.clone(),
        width: 20,
        height: 6,
    };
    let content = Rc::new(RefCell::new(vec!["first".into(), "second".into()]));
    let mut tui = Tui::with_terminal(terminal);
    tui.root.add_child(Lines(content.clone()));

    tui.start().unwrap();
    content.borrow_mut()[1] = "changed".into();
    tui.request_render(false).unwrap();
    tui.stop().unwrap();

    let output = writes.lock().unwrap().join("");
    assert!(output.contains("first\x1b[0m"));
    assert!(output.contains("\x1b[2Kchanged"));
    assert!(!output.contains("\x1b[2J"));
}

#[test]
fn appended_lines_scroll_instead_of_overwriting_the_previous_bottom_line() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let terminal = TestTerminal {
        writes: writes.clone(),
        width: 20,
        height: 2,
    };
    let content = Rc::new(RefCell::new(vec!["one".into(), "two".into()]));
    let mut tui = Tui::with_terminal(terminal);
    tui.root.add_child(Lines(content.clone()));
    tui.start().unwrap();
    writes.lock().unwrap().clear();

    content.borrow_mut().push("three".into());
    tui.request_render(false).unwrap();

    let output = writes.lock().unwrap().join("");
    assert!(output.contains("\x1b[?2026h\r\n\x1b[2Kthree"));
    assert!(!output.contains("\x1b[1B\r\x1b[2Kthree"));
}

#[test]
fn logical_cursor_marker_is_removed_and_positioned() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let terminal = TestTerminal {
        writes: writes.clone(),
        width: 20,
        height: 6,
    };
    let mut tui = Tui::with_terminal(terminal);
    tui.root.add_child(Lines(Rc::new(RefCell::new(vec![format!(
        "abc{CURSOR_MARKER}def"
    )]))));
    tui.start().unwrap();

    let output = writes.lock().unwrap().join("");
    assert!(!output.contains(CURSOR_MARKER));
    assert!(output.contains("\x1b[4G"));
}
