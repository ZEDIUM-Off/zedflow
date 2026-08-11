use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use zedflow_coding_agent::output_guard::OutputGuard;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn non_interactive_protocol_output_stays_on_raw_stdout() {
    let stdout = Capture::default();
    let stderr = Capture::default();
    let guard = OutputGuard::new(stdout.clone(), stderr.clone());

    guard.take_over_stdout();
    guard.write_stdout(b"startup chatter").unwrap();
    guard.write_raw_stdout("{\"type\":\"result\"}\n");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(guard.flush_raw_stdout())
        .unwrap();

    assert_eq!(&*stdout.0.lock().unwrap(), b"{\"type\":\"result\"}\n");
    assert_eq!(&*stderr.0.lock().unwrap(), b"startup chatter");
}
