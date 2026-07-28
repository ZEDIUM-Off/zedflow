use std::{
    cell::RefCell,
    collections::VecDeque,
    io,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use zedflow_tui::{
    CURSOR_MARKER, Component, OverlayAnchor, OverlayOptions, SizeValue, Tui,
    terminal::{Terminal, TerminalEvent},
    terminal_image::{KittyOptions, delete_kitty_image, encode_kitty},
};

struct Lines(Rc<RefCell<Vec<String>>>);
impl Component for Lines {
    fn render(&self, _: usize) -> Vec<String> {
        self.0.borrow().clone()
    }
    fn handle_input(&mut self, data: &str) {
        self.0.borrow_mut().push(data.to_owned());
    }
}

struct TestTerminal {
    writes: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<VecDeque<TerminalEvent>>>,
    width: u16,
    height: u16,
}
impl Terminal for TestTerminal {
    fn start(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn poll_event(&mut self, _: Duration) -> io::Result<Option<TerminalEvent>> {
        Ok(self.events.lock().unwrap().pop_front())
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
fn terminal_events_are_dispatched_and_rendered_on_the_owning_thread() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(VecDeque::from([
        TerminalEvent::Input("native".into()),
        TerminalEvent::Resize,
    ])));
    let content = Rc::new(RefCell::new(Vec::new()));
    let mut tui = Tui::with_terminal(TestTerminal {
        writes: writes.clone(),
        events,
        width: 20,
        height: 6,
    });
    tui.root.add_child(Lines(content.clone()));

    tui.start().unwrap();
    assert_eq!(tui.process_events(Duration::ZERO).unwrap(), 2);

    assert_eq!(&*content.borrow(), &["native"]);
    assert!(writes.lock().unwrap().join("").contains("native"));
    tui.stop().unwrap();
}

#[test]
fn lifecycle_performs_first_and_differential_renders() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let terminal = TestTerminal {
        writes: writes.clone(),
        events: Arc::new(Mutex::new(VecDeque::new())),
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
        events: Arc::new(Mutex::new(VecDeque::new())),
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
        events: Arc::new(Mutex::new(VecDeque::new())),
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

#[test]
fn kitty_images_are_deleted_and_redrawn_when_a_reserved_row_changes() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let image = encode_kitty(
        "AAAA",
        &KittyOptions {
            columns: Some(2),
            rows: Some(2),
            image_id: Some(88),
            move_cursor: Some(false),
        },
    );
    let content = Rc::new(RefCell::new(vec![image.clone(), String::new()]));
    let mut tui = Tui::with_terminal(TestTerminal {
        writes: writes.clone(),
        events: Arc::new(Mutex::new(VecDeque::new())),
        width: 20,
        height: 6,
    });
    tui.root.add_child(Lines(content.clone()));
    tui.start().unwrap();
    writes.lock().unwrap().clear();

    content.borrow_mut()[1] = "covered".into();
    tui.request_render(false).unwrap();

    let output = writes.lock().unwrap().join("");
    let delete = output.find(&delete_kitty_image(88)).unwrap();
    let redraw = output.find(&image).unwrap();
    assert!(delete < redraw);
    assert!(!output.contains("\x1b[2J"));
    assert!(!output.contains(&(image.clone() + "\x1b[0m")));

    writes.lock().unwrap().clear();
    *content.borrow_mut() = vec!["plain".into()];
    tui.request_render(true).unwrap();
    let output = writes.lock().unwrap().join("");
    assert!(output.find(&delete_kitty_image(88)).unwrap() < output.find("\x1b[2J").unwrap());
}

#[test]
fn overlays_do_not_composite_over_kitty_image_rows() {
    let image = encode_kitty(
        "AAAA",
        &KittyOptions {
            image_id: Some(7),
            ..KittyOptions::default()
        },
    );
    let mut tui = Tui::new();
    tui.root
        .add_child(Lines(Rc::new(RefCell::new(vec![image.clone()]))));
    tui.show_overlay_with_options(
        Lines(Rc::new(RefCell::new(vec!["overlay".into()]))),
        OverlayOptions {
            width: Some(SizeValue::Cells(7)),
            anchor: OverlayAnchor::TopLeft,
            ..OverlayOptions::default()
        },
    );

    assert_eq!(tui.render_frame(20, 1)[0], image);
}
