#[test]
fn startup_prompt_exposes_visible_tools() {
    let prompt = zedflow_coding_agent::system_prompt::build_system_prompt(&Default::default());
    assert!(prompt.contains("Available tools:\n(none)"));
}
