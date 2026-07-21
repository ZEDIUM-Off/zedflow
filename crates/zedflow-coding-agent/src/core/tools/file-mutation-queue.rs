//! Per-file serialization for mutation tools.

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::Mutex as AsyncMutex;

fn queues() -> &'static Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>> {
    static QUEUES: OnceLock<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    QUEUES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mutation_key(file_path: &Path) -> io::Result<PathBuf> {
    let resolved = std::path::absolute(file_path)?;
    match std::fs::canonicalize(&resolved) {
        Ok(path) => Ok(path),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(resolved)
        }
        Err(error) => Err(error),
    }
}

/// Run one operation at a time for canonical aliases of the same file.
pub async fn with_file_mutation_queue<P, F, Fut, T>(file_path: P, operation: F) -> io::Result<T>
where
    P: AsRef<Path>,
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let key = mutation_key(file_path.as_ref())?;
    let queue = {
        let mut queues = queues()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            queues
                .entry(key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
        )
    };

    let guard = Arc::clone(&queue).lock_owned().await;
    let result = operation().await;
    drop(guard);

    let mut queues = queues()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if Arc::strong_count(&queue) == 2
        && queues
            .get(&key)
            .is_some_and(|current| Arc::ptr_eq(current, &queue))
    {
        queues.remove(&key);
    }

    Ok(result)
}
