use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
fn warnings() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}
pub fn warn_deprecation(message: impl AsRef<str>) {
    let message = message.as_ref();
    let mut set = warnings().lock().unwrap();
    if set.insert(message.to_owned()) {
        eprintln!("Deprecation warning: {message}");
    }
}
pub fn clear_deprecation_warnings_for_tests() {
    warnings().lock().unwrap().clear();
}
