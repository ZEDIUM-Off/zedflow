use zedflow_coding_agent::prompt_templates::{parse_command_args, substitute_args};
#[test]
fn templates_support_defaults_and_argument_slices() {
    let args = parse_command_args("one 'two words' three");
    assert_eq!(
        substitute_args("${2:-fallback}: $@:2:2", &args),
        "two words: two words three"
    );
}
