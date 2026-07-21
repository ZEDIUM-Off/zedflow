use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::{Notify, mpsc};
use zedflow_coding_agent::file_mutation_queue::with_file_mutation_queue;
use zedflow_coding_agent::output_accumulator::{
    OutputAccumulator, OutputAccumulatorOptions, OutputSnapshotOptions,
};
use zedflow_coding_agent::path_utils::{expand_path, resolve_read_path, resolve_read_path_async};
use zedflow_coding_agent::truncate::{
    TruncatedBy, TruncationOptions, format_size, truncate_head, truncate_line, truncate_tail,
};

fn temp_dir(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "zedflow-tool-primitives-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn paths_expand_and_try_macos_filename_variants() {
    let home = std::env::var_os("HOME").unwrap();
    assert_eq!(
        expand_path("~/file.txt").unwrap(),
        PathBuf::from(home).join("file.txt")
    );
    assert_eq!(
        expand_path("@~draft.md").unwrap(),
        PathBuf::from("~draft.md")
    );
    assert_eq!(expand_path("a\u{00a0}b").unwrap(), PathBuf::from("a b"));

    let dir = temp_dir("paths");
    let macos_name = "Capture d\u{2019}e\u{301}cran.txt";
    fs::write(dir.join(macos_name), "content").unwrap();
    let supplied = "Capture d'écran.txt";
    assert_eq!(
        resolve_read_path(supplied, &dir).unwrap(),
        dir.join(macos_name)
    );
    assert_eq!(
        tokio_test(resolve_read_path_async(supplied, &dir)).unwrap(),
        dir.join(macos_name)
    );
    let screenshot = "Screenshot 10.00\u{202f}AM.png";
    fs::write(dir.join(screenshot), "content").unwrap();
    assert_eq!(
        resolve_read_path("Screenshot 10.00 AM.png", &dir).unwrap(),
        dir.join(screenshot)
    );

    let greek_nfd = "\u{03b1}\u{0301}\u{03bb}\u{03c6}\u{03b1}.txt";
    let greek_nfc = "\u{03ac}\u{03bb}\u{03c6}\u{03b1}.txt";
    fs::write(dir.join(greek_nfd), "content").unwrap();
    assert_eq!(
        resolve_read_path(greek_nfc, &dir).unwrap(),
        dir.join(greek_nfd)
    );
    assert_eq!(
        tokio_test(resolve_read_path_async(greek_nfc, &dir)).unwrap(),
        dir.join(greek_nfd)
    );
    fs::remove_dir_all(dir).unwrap();
}

fn tokio_test<F: Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

use std::future::Future;

#[test]
fn truncation_matches_line_and_utf8_byte_limits() {
    let head = truncate_head(
        "ééé\nnext",
        TruncationOptions {
            max_lines: 10,
            max_bytes: 5,
        },
    );
    assert_eq!(head.content, "");
    assert!(head.first_line_exceeds_limit);
    assert_eq!(head.truncated_by, Some(TruncatedBy::Bytes));

    let tail = truncate_tail(
        "first\nA€BC",
        TruncationOptions {
            max_lines: 10,
            max_bytes: 4,
        },
    );
    assert_eq!(tail.content, "BC");
    assert!(tail.last_line_partial);
    assert_eq!(tail.output_bytes, 2);

    assert_eq!(
        truncate_line("abcdef", 3),
        ("abc... [truncated]".into(), true)
    );
    assert_eq!(format_size(1536), "1.5KB");
}

#[tokio::test]
async fn accumulator_decodes_chunks_keeps_tail_and_persists_full_bytes() {
    let mut output = OutputAccumulator::new(OutputAccumulatorOptions {
        max_lines: 2,
        max_bytes: 20,
        temp_file_prefix: "zedflow-output-test".into(),
    });
    output.append([0xe2, 0x82]).unwrap();
    output.append([0xac]).unwrap();
    output.append(b"\nsecond\nthird").unwrap();
    output.finish().unwrap();

    let snapshot = output.snapshot(OutputSnapshotOptions::default()).unwrap();
    assert_eq!(snapshot.content, "second\nthird");
    assert_eq!(snapshot.truncation.total_lines, 3);
    assert_eq!(snapshot.truncation.truncated_by, Some(TruncatedBy::Lines));
    assert_eq!(output.last_line_bytes(), 5);

    let full_path = snapshot.full_output_path.unwrap();
    output.close_temp_file().await.unwrap();
    assert_eq!(fs::read(&full_path).unwrap(), "€\nsecond\nthird".as_bytes());
    fs::remove_file(full_path).unwrap();
}

#[tokio::test]
async fn mutation_queue_serializes_canonical_aliases_but_not_other_files() {
    let dir = temp_dir("queue");
    let target = dir.join("target.txt");
    let alias = dir.join("alias.txt");
    let other = dir.join("other.txt");
    fs::write(&target, "x").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, &alias).unwrap();

    let release = Arc::new(Notify::new());
    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let first_release = Arc::clone(&release);
    let first_tx = events_tx.clone();
    let first_path = target.clone();
    let first = tokio::spawn(async move {
        with_file_mutation_queue(first_path, || async move {
            first_tx.send("first:start").unwrap();
            first_release.notified().await;
            first_tx.send("first:end").unwrap();
        })
        .await
        .unwrap();
    });
    assert_eq!(events_rx.recv().await, Some("first:start"));

    let alias_tx = events_tx.clone();
    let alias_task = tokio::spawn(async move {
        with_file_mutation_queue(alias, || async move {
            alias_tx.send("alias:start").unwrap();
        })
        .await
        .unwrap();
    });
    let other_tx = events_tx.clone();
    let other_task = tokio::spawn(async move {
        with_file_mutation_queue(other, || async move {
            other_tx.send("other:start").unwrap();
        })
        .await
        .unwrap();
    });

    assert_eq!(events_rx.recv().await, Some("other:start"));
    assert!(events_rx.try_recv().is_err());
    release.notify_one();
    assert_eq!(events_rx.recv().await, Some("first:end"));
    assert_eq!(events_rx.recv().await, Some("alias:start"));

    first.await.unwrap();
    alias_task.await.unwrap();
    other_task.await.unwrap();
    fs::remove_dir_all(dir).unwrap();
}
