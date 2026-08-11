use zedflow_coding_agent::modes::print_mode::{piped_initial_message, should_run_print};
use zedflow_coding_agent::{AssistantResult, render_print_result};
#[test]
fn print_mode_returns_success_and_newline_for_text() {
    assert_eq!(
        render_print_result(&AssistantResult::Text("done".into())),
        (0, "done\n".into())
    );
}

#[test]
fn redirected_stream_selects_print_and_stdin_becomes_initial_prompt() {
    assert!(should_run_print(false, false, true));
    assert!(should_run_print(false, true, false));
    assert_eq!(
        piped_initial_message("  hello\n".into()).as_deref(),
        Some("hello")
    );
}
