use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const BRACKETED_PASTE_ON: &[u8] = b"\x1b[?2004h";
const BRACKETED_PASTE_OFF: &[u8] = b"\x1b[?2004l";
const RESTORED: &[u8] = b"ZEDFLOW_TERMIOS_RESTORED";

#[test]
fn exit_command_restores_terminal() {
    assert_restores_terminal(b"/exit\r");
}

#[test]
fn ctrl_c_restores_terminal() {
    assert_restores_terminal(b"\x03");
}

#[test]
fn configuration_error_leaves_terminal_restored() {
    let binary = env!("CARGO_BIN_EXE_zedflow-coding-agent").replace('\'', "'\\''");
    let command = format!(
        "before=$(stty -g); '{binary}' config; code=$?; after=$(stty -g); \
         [ \"$before\" = \"$after\" ] && printf '{marker}\\n'; exit $code",
        marker = String::from_utf8_lossy(RESTORED),
    );
    let output = Command::new("script")
        .args(["-qfec", &command, "/dev/null"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "configuration error unexpectedly succeeded"
    );
    assert!(
        output
            .stdout
            .windows(RESTORED.len())
            .any(|bytes| bytes == RESTORED),
        "termios changed on error: {:?}",
        output.stdout
    );
}

fn assert_restores_terminal(input: &[u8]) {
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
    let mut output = Vec::new();
    while !output
        .windows(BRACKETED_PASTE_ON.len())
        .any(|bytes| bytes == BRACKETED_PASTE_ON)
    {
        output.extend(
            receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("interactive process did not start"),
        );
    }
    child.stdin.as_mut().unwrap().write_all(input).unwrap();
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
        output
            .windows(BRACKETED_PASTE_OFF.len())
            .any(|bytes| bytes == BRACKETED_PASTE_OFF),
        "bracketed paste was not disabled: {output:?}"
    );
    assert!(
        output
            .windows(RESTORED.len())
            .any(|bytes| bytes == RESTORED),
        "termios was not restored: {output:?}"
    );
}
