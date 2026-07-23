//! RPC-only entry-point helpers.

use crate::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    cli::{Args, parse_args},
    config,
    core::settings_manager::SettingsManager,
    defaults::DEFAULT_THINKING_LEVEL,
    modes::rpc::rpc_mode::run_rpc_loop_with_runtime,
};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, repo_utils::to_shared_session},
    types::{AgentHarnessOptions, Session as AgentSessionTrait},
};
use zedflow_ai::{Model, Models, providers::all::builtin_models};

#[must_use]
pub fn rpc_args(args: &[String]) -> crate::cli::Args {
    let mut combined = vec!["--mode".to_owned(), "rpc".to_owned()];
    combined.extend_from_slice(args);
    parse_args(combined)
}

pub fn run<R: BufRead, W: Write + Send + 'static>(reader: R, writer: W) -> io::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    run_with_args(&args, reader, writer)
}

fn run_with_args<R: BufRead, W: Write + Send + 'static>(
    args: &[String],
    reader: R,
    writer: W,
) -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    let cwd_string = cwd.to_string_lossy().into_owned();
    let parsed = rpc_args(args);
    let settings = SettingsManager::create(&cwd, config::get_agent_dir());
    let models = builtin_models();
    let model = configured_model(&parsed, &settings, &models);
    let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd_string));
    let session = Arc::new(to_shared_session(Arc::new(
        InMemorySessionStorage::default(),
    ))) as Arc<dyn AgentSessionTrait>;
    let session = AgentSession::new(AgentHarnessOptions {
        env,
        session,
        models,
        tools: None,
        resources: None,
        system_prompt: parsed
            .system_prompt
            .map(zedflow_agent::harness::types::SystemPrompt::Text),
        stream_options: None,
        model,
        thinking_level: Some(parsed.thinking.unwrap_or(DEFAULT_THINKING_LEVEL)),
        active_tool_names: None,
        steering_mode: Some(queue_mode(&settings.get_steering_mode())),
        follow_up_mode: Some(queue_mode(&settings.get_follow_up_mode())),
    })
    .map_err(|error| io::Error::other(error.to_string()))?;
    let runtime = AgentSessionRuntime::new(session, cwd_string);
    run_rpc_loop_with_runtime(reader, writer, &runtime)
}

fn queue_mode(value: &str) -> zedflow_agent::types::QueueMode {
    match value {
        "all" => zedflow_agent::types::QueueMode::All,
        _ => zedflow_agent::types::QueueMode::OneAtATime,
    }
}

fn configured_model(args: &Args, settings: &SettingsManager, models: &Models) -> Model {
    let configured_provider = settings.get_default_provider();
    let configured_model = settings.get_default_model();
    let provider = args
        .provider
        .as_deref()
        .or(configured_provider.as_deref())
        .map(str::to_owned);
    let requested = args.model.as_deref().or(configured_model.as_deref());

    requested
        .and_then(|requested| {
            let (provider, id) = requested
                .split_once('/')
                .map_or((provider.as_deref(), requested), |(provider, id)| {
                    (Some(provider), id)
                });
            provider
                .and_then(|provider| models.get_model(provider, id))
                .or_else(|| {
                    models
                        .get_models(None)
                        .into_iter()
                        .find(|model| model.id == id)
                })
        })
        .or_else(|| {
            provider.and_then(|provider| models.get_models(Some(&provider)).into_iter().next())
        })
        .or_else(|| models.get_models(None).into_iter().next())
        .unwrap_or_default()
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

    #[test]
    fn configured_model_uses_rpc_provider_and_model_flags() {
        let args = rpc_args(&[
            "--provider".to_owned(),
            "openai".to_owned(),
            "--model".to_owned(),
            "gpt-4".to_owned(),
        ]);
        let settings = SettingsManager::in_memory(Default::default());
        let model = configured_model(&args, &settings, &builtin_models());

        assert_eq!(model.provider, "openai");
        assert_eq!(model.id, "gpt-4");
    }
}
