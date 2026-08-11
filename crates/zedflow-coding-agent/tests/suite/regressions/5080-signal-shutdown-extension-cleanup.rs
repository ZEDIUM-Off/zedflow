use zedflow_coding_agent::tools_index::{
    create_all_tools, create_coding_tools, create_read_only_tools,
};

#[test]
fn built_in_tool_groups_are_constructed_without_losing_entries() {
    assert_eq!(create_coding_tools(".").len(), 4);
    assert_eq!(create_read_only_tools(".").len(), 4);
    assert_eq!(create_all_tools(".").len(), 7);
}
