use std::{
    io::{self, Write},
    sync::{Arc, Mutex, mpsc},
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

struct BlocksFirstFlush {
    output: Capture,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    blocked: bool,
}

impl Write for BlocksFirstFlush {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.blocked {
            self.blocked = true;
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
        }
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

#[tokio::test]
async fn waiting_drains_writes_queued_while_awaiting_a_barrier() {
    let stdout = Capture::default();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let guard = Arc::new(OutputGuard::new(
        BlocksFirstFlush {
            output: stdout.clone(),
            entered: entered_tx,
            release: release_rx,
            blocked: false,
        },
        io::sink(),
    ));

    guard.write_raw_stdout("first");
    let waiting = tokio::spawn({
        let guard = guard.clone();
        async move { guard.wait_for_raw_stdout_backpressure().await }
    });
    tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
        .await
        .unwrap();
    guard.write_raw_stdout(" second");
    release_tx.send(()).unwrap();
    waiting.await.unwrap().unwrap();

    assert_eq!(&*stdout.0.lock().unwrap(), b"first second");
}
