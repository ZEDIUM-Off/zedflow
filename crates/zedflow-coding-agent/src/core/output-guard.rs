use std::{
    io::{self, Write},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

const RAW_STDOUT_RETRY_DELAY: Duration = Duration::from_millis(10);

enum Message {
    Write(String),
    Barrier(mpsc::Sender<io::Result<()>>),
}

/// Owns the output streams used by an interactive coding-agent session.
///
/// Normal stdout is redirected to stderr while takeover is active. Raw stdout
/// writes keep their submission order and are completed by a single worker.
pub struct OutputGuard {
    stdout: Arc<Mutex<Box<dyn Write + Send>>>,
    stderr: Arc<Mutex<Box<dyn Write + Send>>>,
    taken_over: Mutex<bool>,
    raw_tx: mpsc::Sender<Message>,
}

impl Default for OutputGuard {
    fn default() -> Self {
        Self::new(io::stdout(), io::stderr())
    }
}

impl OutputGuard {
    pub fn new(stdout: impl Write + Send + 'static, stderr: impl Write + Send + 'static) -> Self {
        let stdout: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(Box::new(stdout)));
        let (raw_tx, raw_rx) = mpsc::channel();
        let raw_stdout = stdout.clone();
        thread::spawn(move || raw_writer(raw_stdout, raw_rx));
        Self {
            stdout,
            stderr: Arc::new(Mutex::new(Box::new(stderr))),
            taken_over: Mutex::new(false),
            raw_tx,
        }
    }

    pub fn take_over_stdout(&self) {
        *self.taken_over.lock().unwrap() = true;
    }

    pub fn restore_stdout(&self) {
        *self.taken_over.lock().unwrap() = false;
    }

    pub fn is_stdout_taken_over(&self) -> bool {
        *self.taken_over.lock().unwrap()
    }

    /// Writes through the currently visible stdout stream.
    pub fn write_stdout(&self, bytes: &[u8]) -> io::Result<()> {
        if self.is_stdout_taken_over() {
            self.stderr.lock().unwrap().write_all(bytes)
        } else {
            self.stdout.lock().unwrap().write_all(bytes)
        }
    }

    /// Queues text for the original stdout, even during takeover.
    pub fn write_raw_stdout(&self, text: impl Into<String>) {
        let text = text.into();
        if !text.is_empty() {
            let _ = self.raw_tx.send(Message::Write(text));
        }
    }

    pub async fn wait_for_raw_stdout_backpressure(&self) -> io::Result<()> {
        let (tx, rx) = mpsc::channel();
        self.raw_tx
            .send(Message::Barrier(tx))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "raw stdout worker stopped"))?;
        tokio::task::spawn_blocking(move || {
            rx.recv().unwrap_or_else(|_| {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "raw stdout worker stopped",
                ))
            })
        })
        .await
        .map_err(io::Error::other)?
    }

    pub async fn flush_raw_stdout(&self) -> io::Result<()> {
        self.wait_for_raw_stdout_backpressure().await?;
        self.stdout.lock().unwrap().flush()
    }
}

fn raw_writer(stdout: Arc<Mutex<Box<dyn Write + Send>>>, rx: mpsc::Receiver<Message>) {
    let mut error: Option<(io::ErrorKind, String)> = None;
    for message in rx {
        match message {
            Message::Write(text) if error.is_none() => {
                if let Err(write_error) =
                    write_with_retry(&mut **stdout.lock().unwrap(), text.as_bytes())
                {
                    error = Some((write_error.kind(), write_error.to_string()));
                }
            }
            Message::Write(_) => {}
            Message::Barrier(reply) => {
                let result = match &error {
                    Some((kind, message)) => Err(io::Error::new(*kind, message.clone())),
                    None => stdout.lock().unwrap().flush(),
                };
                let _ = reply.send(result);
            }
        }
    }
}

fn write_with_retry(writer: &mut dyn Write, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        match writer.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(written) => bytes = &bytes[written..],
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.raw_os_error() == Some(105) =>
            {
                // Linux ENOBUFS; EAGAIN/EWOULDBLOCK map to WouldBlock.
                thread::sleep(RAW_STDOUT_RETRY_DELAY)
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
