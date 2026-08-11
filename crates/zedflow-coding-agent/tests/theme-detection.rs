#[test]
fn known_highlight_languages_are_deterministic() {
    assert!(zedflow_coding_agent::utils::syntax_highlight::supports_language("typescript"));
    assert!(!zedflow_coding_agent::utils::syntax_highlight::supports_language("unknown"));
}
