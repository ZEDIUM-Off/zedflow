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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{
        InMemorySessionStorage, InMemorySessionStorageOptions, JsonlSessionRepo,
        JsonlSessionStorage, JsonlSessionStorageCreateOptions, repo_utils::to_shared_session,
    },
    types::{
        AgentHarnessOptions, FileSystem, JsonlSessionListOptions, Session as AgentSessionTrait,
        SessionMetadata,
    },
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
    let session = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| io::Error::other(error.to_string()))?
        .block_on(create_session(&parsed, &cwd, &cwd_string, &settings, &env))
        .map_err(io::Error::other)?;
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

async fn create_session(
    args: &Args,
    cwd: &Path,
    cwd_string: &str,
    settings: &SettingsManager,
    env: &Arc<NodeExecutionEnv>,
) -> Result<Arc<dyn AgentSessionTrait>, String> {
    if args.no_session {
        let storage = if let Some(id) = args.session_id.clone() {
            InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
                entries: None,
                metadata: Some(SessionMetadata {
                    id,
                    created_at: zedflow_agent::harness::session::create_timestamp(),
                }),
            }))
            .map_err(|error| error.to_string())?
        } else {
            InMemorySessionStorage::default()
        };
        return Ok(Arc::new(to_shared_session(Arc::new(storage))) as Arc<dyn AgentSessionTrait>);
    }

    let session_root = args
        .session_dir
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| settings.get_session_dir())
        .unwrap_or_else(config::get_sessions_dir);
    let repo = JsonlSessionRepo::new(
        Arc::clone(env) as Arc<dyn FileSystem>,
        session_root.to_string_lossy().into_owned(),
    );

    if let Some(session_arg) = args.session.as_deref() {
        let path = if session_arg.contains('/')
            || session_arg.contains('\\')
            || session_arg.ends_with(".jsonl")
        {
            let path = Path::new(session_arg);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        } else {
            let sessions = repo
                .list(JsonlSessionListOptions {
                    cwd: Some(cwd_string.to_owned()),
                })
                .await
                .map_err(|error| error.to_string())?;
            let metadata = sessions
                .into_iter()
                .find(|session| {
                    session.base.id == session_arg || session.base.id.starts_with(session_arg)
                })
                .ok_or_else(|| format!("No session found matching '{session_arg}'"))?;
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        };

        let path = path.to_string_lossy().into_owned();
        if env
            .exists(&path, None)
            .await
            .map_err(|error| error.to_string())?
        {
            let metadata =
                zedflow_agent::harness::session::load_jsonl_session_metadata(env.as_ref(), &path)
                    .await
                    .map_err(|error| error.to_string())?;
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        }

        let storage = JsonlSessionStorage::create(
            Arc::clone(env) as Arc<dyn FileSystem>,
            path,
            JsonlSessionStorageCreateOptions {
                cwd: cwd_string.to_owned(),
                session_id: args
                    .session_id
                    .clone()
                    .unwrap_or_else(zedflow_agent::harness::session::create_session_id),
                parent_session_path: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        return Ok(Arc::new(to_shared_session(Arc::new(storage))) as Arc<dyn AgentSessionTrait>);
    }

    let sessions = repo
        .list(JsonlSessionListOptions {
            cwd: Some(cwd_string.to_owned()),
        })
        .await
        .map_err(|error| error.to_string())?;
    if (args.continue_session || args.resume) && sessions.first().is_some() {
        return repo
            .open(sessions.into_iter().next().expect("session exists"))
            .await
            .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
            .map_err(|error| error.to_string());
    }
    if let Some(session_id) = args.session_id.as_deref() {
        if let Some(metadata) = sessions
            .into_iter()
            .find(|session| session.base.id == session_id)
        {
            return repo
                .open(metadata)
                .await
                .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
                .map_err(|error| error.to_string());
        }
    }

    repo.create(zedflow_agent::harness::types::JsonlSessionCreateOptions {
        id: args.session_id.clone(),
        cwd: cwd_string.to_owned(),
        parent_session_path: None,
    })
    .await
    .map(|session| Arc::new(session) as Arc<dyn AgentSessionTrait>)
    .map_err(|error| error.to_string())
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

        run_with_args(
            &["--no-session".to_owned()],
            input,
            SharedWriter(Arc::clone(&output)),
        )
        .expect("RPC runtime should start");

        let output = output.lock().expect("writer lock");
        let response: serde_json::Value =
            serde_json::from_slice(&output).expect("valid JSON response");
        assert_eq!(response["id"], "state-1");
        assert_eq!(response["command"], "get_state");
        assert_eq!(response["success"], true);
        assert!(response["data"].is_object());
    }

    #[test]
    fn session_dir_flag_creates_a_persistent_session() {
        let root = std::env::temp_dir().join(format!(
            "zedflow-rpc-session-{}",
            zedflow_agent::harness::session::create_session_id()
        ));
        let cwd = std::env::current_dir().expect("current directory");
        let cwd_string = cwd.to_string_lossy().into_owned();
        let settings = SettingsManager::in_memory(Default::default());
        let args = rpc_args(&[
            "--session-dir".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
        let env = Arc::new(NodeExecutionEnv::with_cwd(&cwd_string));
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(create_session(&args, &cwd, &cwd_string, &settings, &env))
            .expect("persistent session");

        assert!(
            std::fs::read_dir(root)
                .expect("session root")
                .flatten()
                .any(|entry| entry.path().is_dir())
        );
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
