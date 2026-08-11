use std::sync::{Arc, Mutex};

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, Session},
    types::{AgentHarnessOptions, Session as SessionTrait},
};
use zedflow_ai::{Model, Models};
use zedflow_coding_agent::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    modes::interactive::{InteractiveMode, interactive_mode},
    slash_commands::{BUILTIN_SLASH_COMMANDS, BuiltinSlashCommandId, parse_builtin_slash_command},
};

#[derive(Clone, Copy)]
enum FixtureResult {
    Success,
    Cancelled,
    Error,
}

struct FixtureService {
    result: FixtureResult,
    calls: Arc<Mutex<Vec<BuiltinSlashCommandId>>>,
}

impl interactive_mode::BuiltinCommandService for FixtureService {
    fn execute(
        &mut self,
        command: BuiltinSlashCommandId,
        _arguments: &str,
    ) -> Result<interactive_mode::BuiltinCommandOutcome, String> {
        self.calls.lock().unwrap().push(command);
        match self.result {
            FixtureResult::Success => Ok(interactive_mode::BuiltinCommandOutcome::Success(
                "success".into(),
            )),
            FixtureResult::Cancelled => Ok(interactive_mode::BuiltinCommandOutcome::Cancelled(
                "cancelled".into(),
            )),
            FixtureResult::Error => Err("error".into()),
        }
    }
}

fn runtime() -> (AgentSessionRuntime, Arc<Session<InMemorySessionStorage>>) {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let store = Arc::new(Session::new(InMemorySessionStorage::default()));
    let session = AgentSession::new(AgentHarnessOptions {
        env: Arc::new(NodeExecutionEnv::with_cwd(&cwd)),
        session: Arc::clone(&store) as Arc<dyn SessionTrait>,
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
    .unwrap();
    (AgentSessionRuntime::new(session, cwd), store)
}

const SERVICE_COMMANDS: &[(&str, BuiltinSlashCommandId)] = &[
    ("/settings", BuiltinSlashCommandId::Settings),
    ("/model claude", BuiltinSlashCommandId::Model),
    ("/scoped-models", BuiltinSlashCommandId::ScopedModels),
    ("/export out.html", BuiltinSlashCommandId::Export),
    ("/import in.jsonl", BuiltinSlashCommandId::Import),
    ("/share", BuiltinSlashCommandId::Share),
    ("/copy", BuiltinSlashCommandId::Copy),
    ("/name example", BuiltinSlashCommandId::Name),
    ("/session", BuiltinSlashCommandId::Session),
    ("/changelog", BuiltinSlashCommandId::Changelog),
    ("/hotkeys", BuiltinSlashCommandId::Hotkeys),
    ("/fork", BuiltinSlashCommandId::Fork),
    ("/clone", BuiltinSlashCommandId::Clone),
    ("/tree", BuiltinSlashCommandId::Tree),
    ("/trust", BuiltinSlashCommandId::Trust),
    ("/login", BuiltinSlashCommandId::Login),
    ("/logout", BuiltinSlashCommandId::Logout),
    ("/new", BuiltinSlashCommandId::New),
    ("/resume", BuiltinSlashCommandId::Resume),
    ("/reload", BuiltinSlashCommandId::Reload),
];

#[test]
fn fixed_inventory_and_exact_parser_match_pi() {
    assert_eq!(BUILTIN_SLASH_COMMANDS.len(), 22);
    for command in BUILTIN_SLASH_COMMANDS {
        assert!(parse_builtin_slash_command(&format!("/{}", command.name)).is_some());
    }
    assert_eq!(
        parse_builtin_slash_command("/exit"),
        Some((BuiltinSlashCommandId::Quit, ""))
    );
    assert!(parse_builtin_slash_command("/settings extra").is_none());
    assert!(parse_builtin_slash_command("/compactly").is_none());
    assert!(parse_builtin_slash_command("/unknown").is_none());
}

#[test]
fn every_service_builtin_has_success_cancel_and_error_without_prompting() {
    for (fixture, expected_status) in [
        (FixtureResult::Success, "success"),
        (FixtureResult::Cancelled, "cancelled"),
        (FixtureResult::Error, "error"),
    ] {
        let (runtime, store) = runtime();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut mode = InteractiveMode::with_runtime(zedflow_tui::ProcessTerminal::new(), runtime);
        mode.set_builtin_command_service(FixtureService {
            result: fixture,
            calls: Arc::clone(&calls),
        });

        for (text, _) in SERVICE_COMMANDS {
            mode.queue_user_input(*text);
            assert_eq!(mode.last_status(), Some(expected_status));
        }

        assert_eq!(
            *calls.lock().unwrap(),
            SERVICE_COMMANDS
                .iter()
                .map(|(_, command)| *command)
                .collect::<Vec<_>>()
        );
        assert_eq!(mode.pending_user_input_count(), 0);
        assert!(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(store.get_entries())
                .is_empty()
        );
    }
}

#[test]
fn quit_and_exit_request_shutdown_without_prompting() {
    for command in ["/quit", "/exit"] {
        let mut mode = InteractiveMode::new();
        mode.queue_user_input(command);
        assert!(mode.exit_requested());
        assert_eq!(mode.pending_user_input_count(), 0);
    }
}
