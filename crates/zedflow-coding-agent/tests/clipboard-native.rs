use zedflow_coding_agent::utils::clipboard_native::load_clipboard_native;

#[test]
fn native_clipboard_loading_is_fallible_not_panicking() {
    let _ = load_clipboard_native();
}
