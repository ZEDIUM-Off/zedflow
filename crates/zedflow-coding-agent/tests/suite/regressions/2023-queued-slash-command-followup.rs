use zedflow_coding_agent::prompt_templates::{parse_command_args, substitute_args};

#[test]
fn prompt_arguments_preserve_quotes_and_expand_slices() {
    let args = parse_command_args("one 'two words' three");
    assert_eq!(substitute_args("$1:$2", &args), "one:two words");
}
