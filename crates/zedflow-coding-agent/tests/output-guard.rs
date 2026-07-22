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

struct BlocksOnce {
    output: Capture,
    blocked: bool,
}

impl Write for BlocksOnce {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.blocked {
            self.blocked = true;
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.output.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn redirects_normal_output_and_orders_retrying_raw_output() {
    let stdout = Capture::default();
    let stderr = Capture::default();
    let guard = OutputGuard::new(
        BlocksOnce {
            output: stdout.clone(),
            blocked: false,
        },
        stderr.clone(),
    );

    guard.take_over_stdout();
    guard.write_stdout(b"diagnostic").unwrap();
    guard.write_raw_stdout("first");
    guard.write_raw_stdout(" second");
    guard.wait_for_raw_stdout_backpressure().await.unwrap();
    guard.restore_stdout();
    guard.write_stdout(b" third").unwrap();

    assert_eq!(&*stdout.0.lock().unwrap(), b"first second third");
    assert_eq!(&*stderr.0.lock().unwrap(), b"diagnostic");
    assert!(!guard.is_stdout_taken_over());
}
