use serde_json::Value;
use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
};

pub struct RpcProcessInstance {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_request_id: u64,
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
        Ok(Self {
            stdin: child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("Failed to create RPC process stdio"))?,
            stdout: BufReader::new(
                child
                    .stdout
                    .take()
                    .ok_or_else(|| io::Error::other("Failed to create RPC process stdio"))?,
            ),
            child,
            next_request_id: 0,
        })
    }
    pub fn send(&mut self, mut command: Value) -> io::Result<Value> {
        self.next_request_id += 1;
        let id = command
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "orchestrator_{}_{}",
                    self.next_request_id,
                    uuid::Uuid::new_v4()
                )
            });
        command["id"] = Value::String(id.clone());
        writeln!(
            self.stdin,
            "{}",
            serde_json::to_string(&command).map_err(io::Error::other)?
        )?;
        self.stdin.flush()?;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "RPC process exited",
                ));
            }
            let value: Value = serde_json::from_str(line.trim()).map_err(io::Error::other)?;
            if value.get("type").and_then(Value::as_str) == Some("response")
                && value.get("id").and_then(Value::as_str) == Some(&id)
            {
                return Ok(value);
            }
        }
    }
    pub fn handle_ui_response(&mut self, response: &Value) -> io::Result<()> {
        writeln!(
            self.stdin,
            "{}",
            serde_json::to_string(response).map_err(io::Error::other)?
        )
    }
    pub fn dispose(&mut self) -> io::Result<()> {
        self.child.kill().or_else(|e| {
            if e.kind() == io::ErrorKind::InvalidInput {
                Ok(())
            } else {
                Err(e)
            }
        })
    }
}
