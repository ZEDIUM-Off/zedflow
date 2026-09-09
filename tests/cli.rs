//! Exercise the launcher and streaming path, separately from direct invoke tests.

use std::process::Command;

use serde_json::{Value, json};

fn run(args: &[&str]) -> anyhow::Result<Vec<Value>> {
    let output = Command::new(env!("CARGO_BIN_EXE_zedflow-lab"))
        .args(args)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[test]
fn memory_cli_demonstrates_sharing_and_isolation() -> anyhow::Result<()> {
    let events = run(&["memory"])?;
    let done: Vec<_> = events
        .iter()
        .filter(|e| e["event"]["type"] == "done")
        .collect();
    assert_eq!(done.len(), 3);
    assert_eq!(
        done[1]["event"]["state"]["matches"],
        json!(["Rust graph workflow runs cargo check before tests."])
    );
    assert_eq!(done[2]["event"]["state"]["matches"], json!([]));
    Ok(())
}

#[test]
fn checkpoint_cli_resumes_in_a_new_process() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let database = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("state.db").display()
    );
    let paused = run(&["checkpoint", "start", "--database", &database])?;
    assert_eq!(paused[0]["status"], "paused");
    let completed = run(&["checkpoint", "resume", "--database", &database])?;
    assert_eq!(completed[0]["status"], "completed");
    assert_eq!(completed[0]["state"]["preparations"], 1);
    assert_eq!(completed[0]["state"]["delivered"], "fixture artifact");
    Ok(())
}
