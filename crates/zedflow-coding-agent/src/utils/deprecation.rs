use std::sync::{Mutex, OnceLock};
fn warnings() -> &'static Mutex<Vec<String>> {
    static W: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(Vec::new()))
}
pub fn warn_deprecation(message: &str) {
    let mut w = warnings().lock().unwrap();
    if !w.iter().any(|x| x == message) {
        w.push(message.to_owned());
        eprintln!("Deprecation warning: {message}");
    }
}
pub fn clear_deprecation_warnings_for_tests() {
    warnings().lock().unwrap().clear();
}
