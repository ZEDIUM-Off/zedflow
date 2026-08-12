use std::sync::Arc;

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{InMemorySessionStorage, Session},
    types::{AgentHarnessOptions, Session as SessionTrait},
};
use zedflow_ai::{Model, Models};
use zedflow_coding_agent::{
    agent_session::AgentSession,
    agent_session_runtime::AgentSessionRuntime,
    modes::interactive::{InteractiveMode, quote_if_needed},
};

#[test]
fn clone_paths_are_shell_quoted_when_needed() {
    assert_eq!(quote_if_needed("repo name"), "'repo name'");
    assert_eq!(quote_if_needed("repo/name"), "repo/name");
}

#[test]
fn clone_uses_the_existing_session_fork_boundary() {
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
    mode.queue_user_input("/clone");
    assert_eq!(mode.last_status(), Some("Nothing to clone yet"));
    assert_eq!(mode.pending_user_input_count(), 0);
}

#[test]
fn malformed_clone_text_preserves_unknown_slash_behavior() {
    let mut mode = InteractiveMode::new();
    mode.queue_user_input("/clone elsewhere");
    assert_eq!(mode.get_user_input().as_deref(), Some("/clone elsewhere"));
}
