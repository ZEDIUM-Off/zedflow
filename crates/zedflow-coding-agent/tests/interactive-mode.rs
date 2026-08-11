use std::{collections::HashSet, sync::Arc};

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, Session},
    types::{AgentHarnessOptions, Session as SessionTrait},
};
use zedflow_ai::{Model, Models};
use zedflow_coding_agent::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    modes::interactive::{
        InteractiveMode, get_path_command_argument, is_anthropic_subscription_auth_key,
        is_api_key_login_provider, quote_if_needed,
    },
};

#[test]
fn path_commands_follow_pi_argument_rules() {
    assert_eq!(
        get_path_command_argument("/import 'path with spaces/session.jsonl'", "/import"),
        Some("path with spaces/session.jsonl".into())
    );
    assert_eq!(
        get_path_command_argument("/import john's/session.jsonl", "/import"),
        Some("john's/session.jsonl".into())
    );
    assert_eq!(
        get_path_command_argument("/important /tmp/session.jsonl", "/import"),
        None
    );
}

#[test]
fn submitted_input_is_driven_through_the_session_runtime() {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let session_store = Arc::new(Session::new(InMemorySessionStorage::default()));
    let session = AgentSession::new(AgentHarnessOptions {
        env: Arc::new(NodeExecutionEnv::with_cwd(&cwd)),
        session: Arc::clone(&session_store) as Arc<dyn SessionTrait>,
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
    let runtime = AgentSessionRuntime::new(session, cwd);
    let mut mode = InteractiveMode::with_runtime(zedflow_tui::ProcessTerminal::new(), runtime);
    mode.queue_user_input("  prompt  ");

    assert!(mode.process_next_user_input().unwrap());
    assert_eq!(mode.pending_user_input_count(), 0);
    assert!(
        !tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(session_store.get_entries())
            .is_empty()
    );
    assert!(!mode.process_next_user_input().unwrap());
}

#[test]
fn session_events_are_rendered_when_the_mode_pumps() {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let session = AgentSession::new(AgentHarnessOptions {
        env: Arc::new(NodeExecutionEnv::with_cwd(&cwd)),
        session: Arc::new(Session::new(InMemorySessionStorage::default())) as Arc<dyn SessionTrait>,
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
    let runtime = AgentSessionRuntime::new(session, cwd);
    let runtime_session = runtime.session();
    let mut mode = InteractiveMode::with_runtime(zedflow_tui::ProcessTerminal::new(), runtime);

    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(runtime_session.next_turn("queued", None))
        .unwrap();
    assert!(!mode.process_next_user_input().unwrap());
    assert!(
        mode.rendered_events()
            .iter()
            .any(|event| event == "queue: 1")
    );
}

#[test]
fn compact_command_is_dispatched_without_becoming_a_prompt() {
    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let session = AgentSession::new(AgentHarnessOptions {
        env: Arc::new(NodeExecutionEnv::with_cwd(&cwd)),
        session: Arc::new(Session::new(InMemorySessionStorage::default())) as Arc<dyn SessionTrait>,
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
    let runtime = AgentSessionRuntime::new(session, cwd);
    let mut mode = InteractiveMode::with_runtime(zedflow_tui::ProcessTerminal::new(), runtime);
    mode.queue_user_input("/compact preserve decisions");

    assert!(mode.process_next_user_input().unwrap());
    assert_eq!(mode.pending_user_input_count(), 0);
    assert_eq!(mode.last_status(), Some("Nothing to compact"));
}

#[test]
fn provider_login_and_auth_key_rules_match_pi() {
    let oauth = HashSet::from(["oauth-provider".to_owned()]);
    let builtins = HashSet::from(["builtin-provider".to_owned()]);
    let api_key_builtins = HashSet::from(["openai".to_owned()]);

    assert!(is_api_key_login_provider(
        "openai",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(!is_api_key_login_provider(
        "builtin-provider",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(!is_api_key_login_provider(
        "oauth-provider",
        &oauth,
        &builtins,
        &api_key_builtins
    ));
    assert!(is_anthropic_subscription_auth_key(Some(
        "sk-ant-oat01-test"
    )));
    assert!(!is_anthropic_subscription_auth_key(Some("sk-ant-api-test")));
}

#[test]
fn shell_quote_only_when_needed() {
    assert_eq!(quote_if_needed("/tmp/session.jsonl"), "/tmp/session.jsonl");
    assert_eq!(quote_if_needed("path with spaces"), "'path with spaces'");
    assert_eq!(quote_if_needed("john's"), "'john'\\''s'");
}
