use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[tokio::test]
async fn mutations_run_through_the_public_queue() {
    let runs = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&runs);
    let result = zedflow_coding_agent::file_mutation_queue::with_file_mutation_queue(
        "queued-file",
        move || async move {
            seen.fetch_add(1, Ordering::SeqCst);
            7
        },
    )
    .await
    .unwrap();
    assert_eq!((result, runs.load(Ordering::SeqCst)), (7, 1));
}
