use zedflow_coding_agent::agent_session_services::AgentSessionServices;

#[test]
fn session_services_preserve_the_bound_paths() {
    let services = AgentSessionServices::new("session-cwd", "agent-dir");
    assert_eq!(services.cwd, std::path::PathBuf::from("session-cwd"));
    assert_eq!(services.agent_dir, std::path::PathBuf::from("agent-dir"));
    assert!(services.diagnostics.is_empty());
}
