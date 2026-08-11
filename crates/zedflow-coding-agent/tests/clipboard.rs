use zedflow_coding_agent::utils::clipboard::copy_to_clipboard;

#[test]
fn clipboard_copy_reports_platform_failure_instead_of_panicking() {
    let _ = copy_to_clipboard("zedflow clipboard regression");
}
