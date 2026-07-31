#[test]
fn coding_tools_have_the_expected_next_request_set() {
    let tools = zedflow_coding_agent::tools_index::create_coding_tools(".");
    assert_eq!(tools.len(), 4);
}
