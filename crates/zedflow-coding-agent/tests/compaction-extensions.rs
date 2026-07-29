use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::json;
use zedflow_coding_agent::extensions::{ExtensionEventKind, ExtensionRunner};

#[test]
fn shutdown_is_once_and_invalidates_extension_context() {
    let mut runner = ExtensionRunner::new(vec![]);
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_out = Arc::clone(&calls);
    runner.on(
        "extension",
        ExtensionEventKind::SessionShutdown,
        Arc::new(move |_, _| {
            calls_out.fetch_add(1, Ordering::SeqCst);
            Ok(Some(json!(null)))
        }),
    );
    runner.shutdown("reload");
    runner.shutdown("quit");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(runner.context().stale);
    assert_eq!(runner.context().generation, 1);
}
