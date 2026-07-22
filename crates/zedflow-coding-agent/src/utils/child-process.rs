use std::process::{Child, Command, ExitStatus, Output, Stdio};

pub type ChildProcess = Child;

pub fn spawn_process(command: &str, args: &[&str]) -> std::io::Result<ChildProcess> {
    Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub fn spawn_process_sync(command: &str, args: &[&str]) -> std::io::Result<Output> {
    Command::new(command).args(args).output()
}

pub fn wait_for_child_process(child: &mut ChildProcess) -> std::io::Result<ExitStatus> {
    child.wait()
}
