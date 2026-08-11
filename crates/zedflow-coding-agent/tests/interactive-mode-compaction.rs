use std::sync::Arc;

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, Session},
    types::{AgentHarnessOptions, Session as SessionTrait},
};
use zedflow_ai::{Model, Models};
use zedflow_coding_agent::{
    agent_session::AgentSession, agent_session_runtime::AgentSessionRuntime,
    modes::interactive::InteractiveMode,
};

#[test]
fn compact_is_dispatched_without_becoming_a_prompt() {
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
    let mut mode = InteractiveMode::with_runtime(
        zedflow_tui::ProcessTerminal::new(),
        AgentSessionRuntime::new(session, cwd),
    );

    mode.queue_user_input("/compact preserve decisions");
    assert!(mode.process_next_user_input().unwrap());
    assert_eq!(mode.last_status(), Some("Nothing to compact"));
    assert!(
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.get_entries())
            .is_empty()
    );
}

#[test]
fn compact_requires_a_complete_command_token() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/compactly");
    assert_eq!(mode.get_user_input().as_deref(), Some("/compactly"));
}
