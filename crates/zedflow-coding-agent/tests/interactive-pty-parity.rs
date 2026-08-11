use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const READY: &[u8] = b"\x1b[?2004h";
const RESTORED: &[u8] = b"ZEDFLOW_PTY_RESTORED";

#[test]
fn editor_input_is_visible_and_abort_restores_the_pty() {
    let binary = env!("CARGO_BIN_EXE_zedflow-coding-agent").replace('\'', "'\\''");
    let command = format!(
        "before=$(stty -g); '{binary}'; code=$?; after=$(stty -g); \
         [ \"$before\" = \"$after\" ] && printf '{marker}\\n'; exit $code",
        marker = String::from_utf8_lossy(RESTORED),
    );
    let mut child = Command::new("script")
        .args(["-qfec", &command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut bytes = [0; 1024];
        while let Ok(count) = stdout.read(&mut bytes) {
            if count == 0 || sender.send(bytes[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut output = recv_until(&receiver, &mut Vec::new(), READY, deadline);
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"parity-check")
        .unwrap();
    thread::sleep(Duration::from_millis(200));
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"\x03\x03")
        .unwrap();
    child.stdin.take();

    let status = child.wait().unwrap();
    reader.join().unwrap();
    for bytes in receiver.try_iter() {
        output.extend(bytes);
    }
    let mut stderr = Vec::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();

    assert!(
        status.success(),
        "process failed: {status:?}; stderr={stderr:?}"
    );
    assert!(
        contains(&output, b"parity-check"),
        "paste was not rendered: {output:?}"
    );
    assert!(
        !contains(&output, b"received"),
        "placeholder output leaked: {output:?}"
    );
    assert!(
        contains(&output, b"\x1b[?2004l"),
        "paste mode leaked: {output:?}"
    );
    assert!(contains(&output, RESTORED), "termios leaked: {output:?}");
}

fn recv_until(
    receiver: &mpsc::Receiver<Vec<u8>>,
    output: &mut Vec<u8>,
    needle: &[u8],
    deadline: Instant,
) -> Vec<u8> {
    while !contains(output, needle) {
        output.extend(
            receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("interactive process did not produce expected PTY bytes"),
        );
    }
    std::mem::take(output)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|bytes| bytes == needle)
}
