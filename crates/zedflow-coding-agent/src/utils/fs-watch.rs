use std::path::Path;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
pub const FS_WATCH_RETRY_DELAY_MS: u64 = 5000;
pub fn close_watcher<T>(watcher: Option<T>) {
    drop(watcher);
}
pub fn watch_with_error_handler<P: AsRef<Path>, F: Fn() + Send + 'static>(
    path: P,
    _listener: F,
    on_error: impl Fn() + Send + 'static,
) -> Option<Receiver<()>> {
    if !path.as_ref().exists() {
        on_error();
        return None;
    }
    let (_tx, rx) = std::sync::mpsc::channel();
    thread::sleep(Duration::from_millis(0));
    Some(rx)
}
