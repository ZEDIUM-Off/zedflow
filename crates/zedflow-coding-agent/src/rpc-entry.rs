//! RPC-only entry-point helpers.

use crate::{
    agent_session::AgentSession, agent_session_runtime::AgentSessionRuntime, cli::parse_args,
    modes::rpc::rpc_mode::run_rpc_loop_with_runtime,
};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, repo_utils::to_shared_session},
    types::{AgentHarnessOptions, Session as AgentSessionTrait},
};
use zedflow_ai::{Model, Models};

#[must_use]
pub fn rpc_args(args: &[String]) -> crate::cli::Args {
    let mut combined = vec!["--mode".to_owned(), "rpc".to_owned()];
    combined.extend_from_slice(args);
    parse_args(combined)
}

pub fn run<R: BufRead, W: Write + Send + 'static>(reader: R, writer: W) -> io::Result<()> {
    let cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd));
    let session = Arc::new(to_shared_session(Arc::new(
        InMemorySessionStorage::default(),
    ))) as Arc<dyn AgentSessionTrait>;
    let session = AgentSession::new(AgentHarnessOptions {
        env,
        session,
        models: Models::default(),
        tools: None,
        resources: None,
        system_prompt: None,
        stream_options: None,
        model: Model::default(),
        thinking_level: None,
        active_tool_names: None,
        steering_mode: None,
        follow_up_mode: None,
    })
    .map_err(|error| io::Error::other(error.to_string()))?;
    let runtime = AgentSessionRuntime::new(session, cwd);
    run_rpc_loop_with_runtime(reader, writer, &runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use std::sync::{Arc, Mutex};

    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("writer lock").extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn run_dispatches_state_commands_to_the_runtime() {
        let input = Cursor::new(
            br#"{"id":"state-1","type":"get_state"}
"#,
        );
        let output = Arc::new(Mutex::new(Vec::new()));

        run(input, SharedWriter(Arc::clone(&output))).expect("RPC runtime should start");

        let output = output.lock().expect("writer lock");
        let response: serde_json::Value =
            serde_json::from_slice(&output).expect("valid JSON response");
        assert_eq!(response["id"], "state-1");
        assert_eq!(response["command"], "get_state");
        assert_eq!(response["success"], true);
        assert!(response["data"].is_object());
    }
}
