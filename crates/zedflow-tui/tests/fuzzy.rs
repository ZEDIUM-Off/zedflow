use zedflow_tui::fuzzy_match;
#[test]
fn fuzzy_matching_is_case_insensitive() {
    assert!(fuzzy_match("ED", "Editor").matches);
    assert!(!fuzzy_match("xyz", "Editor").matches);
}
