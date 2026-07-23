use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

pub const FS_WATCH_RETRY_DELAY_MS: u64 = 5000;

pub struct FsWatcher {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub fn close_watcher(watcher: Option<FsWatcher>) {
    let Some(mut watcher) = watcher else { return };
    watcher.stop.store(true, Ordering::Relaxed);
    if let Some(thread) = watcher.thread.take() {
        let _ = thread.join();
    }
}

pub fn watch_with_error_handler<P, L, E>(path: P, listener: L, on_error: E) -> Option<FsWatcher>
where
    P: AsRef<Path>,
    L: Fn() + Send + 'static,
    E: Fn() + Send + 'static,
{
    let path = path.as_ref().to_owned();
    let mut last_modified = match modified(&path) {
        Ok(value) => value,
        Err(_) => {
            on_error();
            return None;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        while !thread_stop.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(FS_WATCH_RETRY_DELAY_MS));
            if thread_stop.load(Ordering::Relaxed) {
                break;
            }
            match modified(&path) {
                Ok(value) if value != last_modified => {
                    last_modified = value;
                    listener();
                }
                Ok(_) => {}
                Err(_) => {
                    on_error();
                    break;
                }
            }
        }
    });
    Some(FsWatcher {
        stop,
        thread: Some(thread),
    })
}

fn modified(path: &PathBuf) -> std::io::Result<Option<SystemTime>> {
    Ok(path.metadata()?.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn calls_error_handler_when_watch_cannot_start() {
        let errors = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&errors);
        assert!(
            watch_with_error_handler(
                "/definitely/not/a/real/path",
                || {},
                move || {
                    seen.fetch_add(1, Ordering::Relaxed);
                },
            )
            .is_none()
        );
        assert_eq!(errors.load(Ordering::Relaxed), 1);
    }
}
