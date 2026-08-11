use std::sync::{Arc, Mutex};

static AGENT_DIR_LOCK: Mutex<()> = Mutex::new(());
use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, Session},
    types::{AgentHarnessOptions, Session as SessionTrait},
};
use zedflow_ai::{Model, Models};
use zedflow_coding_agent::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    auth_storage::{AuthCredential, AuthStorage},
    config::get_auth_path,
    modes::interactive::InteractiveMode,
    slash_commands::{BUILTIN_SLASH_COMMANDS, BuiltinSlashCommandId, parse_builtin_slash_command},
};

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
fn live_builtins_are_intercepted_before_prompting() {
    let _lock = AGENT_DIR_LOCK.lock().unwrap();
    let agent_dir =
        std::env::temp_dir().join(format!("zedflow-live-builtins-{}", std::process::id()));
    let previous = std::env::var_os("PI_AGENT_DIR");
    unsafe { std::env::set_var("PI_AGENT_DIR", &agent_dir) };
    let (runtime, store) = runtime();
    let mut mode = InteractiveMode::with_runtime(zedflow_tui::ProcessTerminal::new(), runtime);
    for command in BUILTIN_SLASH_COMMANDS {
        if matches!(command.name, "compact" | "quit") {
            continue;
        }
        let input = match command.name {
            "import" => "/import missing.jsonl".to_owned(),
            "name" => "/name example".to_owned(),
            "export" => format!("/export {}", agent_dir.join("export.jsonl").display()),
            "model" => "/model".to_owned(),
            _ => format!("/{}", command.name),
        };
        mode.queue_user_input(input);
        assert_eq!(
            mode.pending_user_input_count(),
            0,
            "{} was queued",
            command.name
        );
    }
    assert!(
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_entries())
            .into_iter()
            .all(|entry| serde_json::to_value(entry)
                .ok()
                .and_then(|entry| entry
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned))
                .is_some_and(|kind| kind == "session_info"))
    );
    match previous {
        Some(value) => unsafe { std::env::set_var("PI_AGENT_DIR", value) },
        None => unsafe { std::env::remove_var("PI_AGENT_DIR") },
    }
    let _ = std::fs::remove_dir_all(agent_dir);
}

#[test]
fn live_selector_opens_for_logout_without_prompting() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/logout");
    mode.run().unwrap();
    assert_eq!(mode.tui_mut().overlay_count(), 1);
    mode.stop().unwrap();
}

#[test]
fn live_logout_selection_mutates_configured_auth_storage() {
    let _lock = AGENT_DIR_LOCK.lock().unwrap();
    let agent_dir =
        std::env::temp_dir().join(format!("zedflow-live-logout-{}", std::process::id()));
    let previous = std::env::var_os("PI_AGENT_DIR");
    unsafe { std::env::set_var("PI_AGENT_DIR", &agent_dir) };
    let mut storage = AuthStorage::create(get_auth_path());
    storage
        .set(
            "fixture",
            AuthCredential::ApiKey {
                key: "secret".into(),
                env: None,
            },
        )
        .unwrap();

    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/logout");
    mode.run().unwrap();
    mode.tui_mut().dispatch_input("\r");
    mode.pump_events(std::time::Duration::ZERO).unwrap();
    mode.stop().unwrap();
    assert!(!AuthStorage::create(get_auth_path()).has("fixture"));

    match previous {
        Some(value) => unsafe { std::env::set_var("PI_AGENT_DIR", value) },
        None => unsafe { std::env::remove_var("PI_AGENT_DIR") },
    }
    let _ = std::fs::remove_dir_all(agent_dir);
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
