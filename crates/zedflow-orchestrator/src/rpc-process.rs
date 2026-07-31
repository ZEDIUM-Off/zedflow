use serde_json::Value;
use std::{
    collections::HashMap,
    env,
    io::{self, BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
};
use tokio::sync::mpsc::UnboundedSender;

struct Inner {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Mutex<HashMap<String, mpsc::Sender<io::Result<Value>>>>,
    subscribers: Mutex<HashMap<u64, UnboundedSender<Value>>>,
    ui_request_handler: Mutex<Option<(u64, UnboundedSender<Value>)>>,
    next_subscriber: AtomicU64,
    exited: AtomicBool,
}

#[derive(Clone)]
pub struct RpcProcessInstance {
    inner: Arc<Inner>,
    next_request_id: Arc<AtomicU64>,
}

impl RpcProcessInstance {
    pub fn new(cwd: &str) -> io::Result<Self> {
        let program = env::var("PI_ORCHESTRATOR_RPC_COMMAND").unwrap_or_else(|_| "pi".into());
        let mut child = Command::new(program)
            .args(["--mode", "rpc"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("Failed to create RPC process stdio"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Failed to create RPC process stdio"))?;
        let inner = Arc::new(Inner {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            subscribers: Mutex::new(HashMap::new()),
            ui_request_handler: Mutex::new(None),
            next_subscriber: AtomicU64::new(0),
            exited: AtomicBool::new(false),
        });
        let reader = inner.clone();
        std::thread::spawn(move || read_stdout(stdout, reader));
        Ok(Self {
            inner,
            next_request_id: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn send(&self, mut command: Value) -> io::Result<Value> {
        if self.inner.exited.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "RPC process is not running",
            ));
        }
        let id = command
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "orchestrator_{}_{}",
                    self.next_request_id.fetch_add(1, Ordering::SeqCst) + 1,
                    uuid::Uuid::new_v4()
                )
            });
        command["id"] = Value::String(id.clone());
        let (tx, rx) = mpsc::channel();
        self.inner.pending.lock().unwrap().insert(id.clone(), tx);
        let write = (|| {
            let mut stdin = self.inner.stdin.lock().unwrap();
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(&command).map_err(io::Error::other)?
            )?;
            stdin.flush()
        })();
        if let Err(error) = write {
            self.inner.pending.lock().unwrap().remove(&id);
            return Err(error);
        }
        rx.recv()
            .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "RPC process exited"))?
    }

    pub fn handle_ui_response(&self, response: &Value) -> io::Result<()> {
        if self.inner.exited.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut stdin = self.inner.stdin.lock().unwrap();
        writeln!(
            stdin,
            "{}",
            serde_json::to_string(response).map_err(io::Error::other)?
        )?;
        stdin.flush()
    }

    pub fn subscribe(
        &self,
        events: UnboundedSender<Value>,
        ui_requests: UnboundedSender<Value>,
    ) -> u64 {
        let id = self.inner.next_subscriber.fetch_add(1, Ordering::SeqCst);
        self.inner.subscribers.lock().unwrap().insert(id, events);
        *self.inner.ui_request_handler.lock().unwrap() = Some((id, ui_requests));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.inner.subscribers.lock().unwrap().remove(&id);
        let mut handler = self.inner.ui_request_handler.lock().unwrap();
        if handler
            .as_ref()
            .is_some_and(|(handler_id, _)| *handler_id == id)
        {
            *handler = None;
        }
    }

    pub fn dispose(&self) -> io::Result<()> {
        self.inner.exited.store(true, Ordering::SeqCst);
        self.inner.child.lock().unwrap().kill().or_else(|e| {
            if e.kind() == io::ErrorKind::InvalidInput {
                Ok(())
            } else {
                Err(e)
            }
        })
    }
}

fn read_stdout(stdout: std::process::ChildStdout, inner: Arc<Inner>) {
    let mut stdout = BufReader::new(stdout);
    let mut line = String::new();
    while stdout
        .read_line(&mut line)
        .ok()
        .filter(|n| *n > 0)
        .is_some()
    {
        let parsed: Value = match serde_json::from_str(line.trim()) {
            Ok(value) => value,
            Err(_) => {
                line.clear();
                continue;
            }
        };
        match parsed.get("type").and_then(Value::as_str) {
            Some("response") => {
                if let Some(id) = parsed.get("id").and_then(Value::as_str) {
                    if let Some(pending) = inner.pending.lock().unwrap().remove(id) {
                        let _ = pending.send(Ok(parsed));
                    }
                }
            }
            Some("extension_ui_request") => {
                if let Some((_, handler)) = inner.ui_request_handler.lock().unwrap().as_ref() {
                    let _ = handler.send(parsed);
                }
            }
            _ => {
                for subscriber in inner.subscribers.lock().unwrap().values() {
                    let _ = subscriber.send(parsed.clone());
                }
            }
        }
        line.clear();
    }
    inner.exited.store(true, Ordering::SeqCst);
    for (_, pending) in inner.pending.lock().unwrap().drain() {
        let _ = pending.send(Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "RPC process exited",
        )));
    }
}
