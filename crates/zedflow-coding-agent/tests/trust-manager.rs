#[test]
fn trust_options_offer_persistent_and_session_decisions() {
    let options =
        zedflow_coding_agent::trust_manager::get_project_trust_options(".", true).unwrap();
    assert!(options.iter().any(|option| option.label == "Trust"));
    assert!(
        options
            .iter()
            .any(|option| option.label == "Do not trust (this session only)")
    );
}
