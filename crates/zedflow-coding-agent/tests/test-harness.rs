#[test]
fn all_built_in_tools_can_be_constructed() {
    assert_eq!(
        zedflow_coding_agent::tools_index::create_all_tools(".").len(),
        zedflow_coding_agent::tools_index::ALL_TOOL_NAMES.len()
    );
}
