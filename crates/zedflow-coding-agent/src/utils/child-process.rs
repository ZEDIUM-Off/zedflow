use std::io;
use std::process::{Child, Command, ExitStatus, Output};

pub fn spawn_process(command: &str, args: &[&str]) -> io::Result<Child> {
    Command::new(command).args(args).spawn()
}
pub fn spawn_process_sync(command: &str, args: &[&str]) -> io::Result<Output> {
    Command::new(command).args(args).output()
}

/// Wait for a child and return its exit code. The standard library's wait also
/// closes inherited pipes, avoiding a detached descendant keeping this call open.
pub fn wait_for_child_process(mut child: Child) -> io::Result<Option<i32>> {
    let status = child.wait()?;
    Ok(exit_code(status))
}
fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code()
}
