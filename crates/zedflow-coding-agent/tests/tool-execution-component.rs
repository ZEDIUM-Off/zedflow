#[test]
fn read_only_tool_set_excludes_mutating_tools() {
    assert_eq!(
        zedflow_coding_agent::tools_index::create_read_only_tools(".").len(),
        4
    );
}
