use zedflow_coding_agent::parse_args;

#[test]
fn parses_print_message_without_treating_flags_as_values() {
    let args = parse_args(["--print", "hello", "--no-tools"]);
    assert!(args.print);
    assert_eq!(args.messages, ["hello"]);
    assert!(args.no_tools);
}
