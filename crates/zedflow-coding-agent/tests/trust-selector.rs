#[test]
fn trust_options_include_rejection() {
    assert!(
        zedflow_coding_agent::trust_manager::get_project_trust_options(".", false)
            .unwrap()
            .iter()
            .any(|option| !option.trusted)
    );
}
