use zedflow_coding_agent::{AssistantResult, render_print_result};
#[test]
fn print_mode_returns_success_and_newline_for_text() {
    assert_eq!(
        render_print_result(&AssistantResult::Text("done".into())),
        (0, "done\n".into())
    );
}
