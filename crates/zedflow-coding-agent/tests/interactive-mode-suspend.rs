use std::{
    cell::RefCell,
    io::{self, BufRead, Write},
    process::{Command, Stdio},
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use zedflow_coding_agent::modes::interactive::{InteractiveMode, InteractiveState};
use zedflow_tui::{Component, ProcessTerminal};

#[derive(Clone)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Inputs(Rc<RefCell<Vec<String>>>);

impl Component for Inputs {
    fn render(&self, _: usize) -> Vec<String> {
        self.0.borrow().clone()
    }

    fn handle_input(&mut self, data: &str) {
        self.0.borrow_mut().push(data.to_owned());
    }
}

#[test]
fn interactive_mode_owns_native_pump_and_restores_terminal_around_editor() {
    const CHILD_ENV: &str = "ZEDFLOW_INTERACTIVE_OWNER_PUMP_CHILD";
    const READY: &str = "zedflow-interactive-ready";
    const STOPPED: &str = "zedflow-interactive-stopped";

    if std::env::var_os(CHILD_ENV).is_some() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let inputs = Rc::new(RefCell::new(Vec::new()));
        let terminal = ProcessTerminal::with_writer(Box::new(SharedWriter(writes.clone())));
        let mut mode = InteractiveMode::with_terminal(terminal);
        mode.tui_mut().root.add_child(Inputs(inputs.clone()));

        mode.run().unwrap();
        assert_eq!(mode.state(), InteractiveState::Running);
        eprintln!("{READY}");
        pump_until_input(&mut mode, &inputs, "a");

        mode.suspend_for_external_editor(|| {
            let output = String::from_utf8_lossy(&writes.lock().unwrap()).into_owned();
            assert!(output.contains("\x1b[?25h"));
            assert!(output.contains("\x1b[?2004l"));
            eprintln!("{STOPPED}");
            thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap();

        assert_eq!(mode.state(), InteractiveState::Running);
        assert_eq!(
            String::from_utf8_lossy(&writes.lock().unwrap())
                .matches("\x1b[?2004h")
                .count(),
            2
        );
        pump_until_input(&mut mode, &inputs, "b");
        assert_eq!(&*inputs.borrow(), &["a", "b"]);
        mode.stop().unwrap();
        assert_eq!(mode.state(), InteractiveState::Stopped);
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "interactive_mode_owns_native_pump_and_restores_terminal_around_editor",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let stderr_thread = thread::spawn(move || {
        for line in io::BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = sender.send(line);
        }
    });
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while let Ok(line) =
            receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            if line.contains(marker) {
                return true;
            }
        }
        false
    };

    if !wait_for(READY) {
        let _ = child.kill();
        let _ = child.wait();
        panic!("child did not report {READY}");
    }
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"a").unwrap();
    stdin.flush().unwrap();

    if !wait_for(STOPPED) {
        let _ = child.kill();
        let _ = child.wait();
        panic!("child did not report {STOPPED}");
    }
    let stdin = child.stdin.as_mut().unwrap();
    stdin.write_all(b"b").unwrap();
    stdin.flush().unwrap();

    let status = child.wait().unwrap();
    stderr_thread.join().unwrap();
    assert!(status.success());
}

fn pump_until_input(mode: &mut InteractiveMode, inputs: &Rc<RefCell<Vec<String>>>, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        mode.pump_events(Duration::from_millis(20)).unwrap();
        if inputs
            .borrow()
            .last()
            .is_some_and(|value| value == expected)
        {
            return;
        }
    }
    panic!("owner pump did not dispatch {expected:?}");
}
