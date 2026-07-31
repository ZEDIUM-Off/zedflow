#[test]
fn supported_syntax_choices_include_common_languages() {
    assert!(zedflow_coding_agent::utils::syntax_highlight::supports_language("json"));
    assert!(zedflow_coding_agent::utils::syntax_highlight::supports_language("bash"));
}
