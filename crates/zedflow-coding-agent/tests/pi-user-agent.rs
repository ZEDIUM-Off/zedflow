use zedflow_coding_agent::utils::pi_user_agent::get_pi_user_agent;
#[test]
fn user_agent_includes_version() {
    assert!(get_pi_user_agent("1.2.3").starts_with("pi/1.2.3 ("));
}
