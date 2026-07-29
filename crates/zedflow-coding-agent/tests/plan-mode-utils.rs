use zedflow_coding_agent::prompt_templates::{parse_command_args, substitute_args};
#[test]
fn plan_arguments_preserve_quoted_words() {
    let args = parse_command_args("one 'two words'");
    assert_eq!(substitute_args("$1 / $2", &args), "one / two words");
}
