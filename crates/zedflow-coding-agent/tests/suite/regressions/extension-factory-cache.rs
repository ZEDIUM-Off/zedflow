#[test]
fn built_in_tool_construction_is_repeatable() {
    assert_eq!(
        zedflow_coding_agent::tools_index::create_all_tools(".").len(),
        7
    );
    assert_eq!(
        zedflow_coding_agent::tools_index::create_all_tools(".").len(),
        7
    );
}
