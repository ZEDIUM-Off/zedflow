#[test]
fn all_tools_include_every_declared_tool_name() {
    assert_eq!(
        zedflow_coding_agent::tools_index::create_all_tools(".").len(),
        7
    );
}
