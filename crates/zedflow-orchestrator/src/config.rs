use std::{env, path::PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub fn is_bun_binary() -> bool {
    false
}
pub fn orchestrator_dir() -> PathBuf {
    if let Some(path) = env::var_os("PI_ORCHESTRATOR_DIR") {
        return path.into();
    }
    let pi_dir = env::var_os("PI_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".pi")))
        .unwrap_or_else(|| PathBuf::from(".pi"));
    pi_dir.join("orchestrator")
}
pub fn auth_path() -> PathBuf {
    let pi_dir = env::var_os("PI_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".pi")))
        .unwrap_or_else(|| PathBuf::from(".pi"));
    pi_dir.join("agent").join("auth.json")
}
pub fn machine_path() -> PathBuf {
    orchestrator_dir().join("machine.json")
}
pub fn instances_path() -> PathBuf {
    orchestrator_dir().join("instances.json")
}
pub fn socket_path() -> PathBuf {
    orchestrator_dir().join("orchestrator.sock")
}
