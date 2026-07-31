use crate::{
    config, storage,
    types::{InstanceRecord, MachineRecord, RadiusRegistration},
};
use serde::Deserialize;
use std::{env, io};

pub const DEFAULT_RADIUS_URL: &str = "https://radius.pi.dev/";
pub fn radius_url() -> String {
    env::var("PI_RADIUS_URL").unwrap_or_else(|_| DEFAULT_RADIUS_URL.into())
}
pub fn radius_orchestrator_base_url() -> String {
    env::var("PI_RADIUS_ORCHESTRATOR_URL")
        .unwrap_or_else(|_| format!("{}/v1/", radius_url().trim_end_matches('/')))
}
#[derive(Deserialize)]
struct AuthFile {
    radius: Option<Credential>,
}
#[derive(Deserialize)]
struct Credential {
    #[serde(rename = "type")]
    kind: String,
    access: Option<String>,
}
pub fn radius_access_token() -> io::Result<String> {
    if let Ok(key) = env::var("PI_RADIUS_API_KEY") {
        if !key.is_empty() {
            return Ok(key);
        }
    }
    let text = std::fs::read_to_string(config::auth_path())?;
    let auth: AuthFile = serde_json::from_str(&text).map_err(io::Error::other)?;
    auth.radius
        .filter(|credential| credential.kind == "oauth")
        .and_then(|credential| credential.access)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Radius credentials are required in ~/.pi/agent/auth.json or PI_RADIUS_API_KEY",
            )
        })
}
pub fn is_radius_enabled() -> bool {
    radius_access_token().is_ok()
}
pub fn compute_backoff_delay_ms(failures: u32) -> u64 {
    (1_000u64
        .saturating_mul(2u64.saturating_pow(failures.saturating_sub(1)))
        .min(30_000))
    .min(30_000)
}
pub struct RadiusPresence {
    machine: Option<MachineRecord>,
}
impl Default for RadiusPresence {
    fn default() -> Self {
        Self { machine: None }
    }
}
impl RadiusPresence {
    pub fn start(&mut self, label: Option<String>) -> io::Result<Option<MachineRecord>> {
        if !is_radius_enabled() {
            return Ok(None);
        }
        let machine = storage::load_machine()?.unwrap_or(MachineRecord {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: "0".into(),
            last_seen_at: None,
            label,
        });
        storage::save_machine(&machine)?;
        self.machine = Some(machine.clone());
        Ok(Some(machine))
    }
    pub fn stop(&mut self) {
        self.machine = None;
    }
    pub fn register_pi(&self, instance: InstanceRecord) -> io::Result<InstanceRecord> {
        if !is_radius_enabled() {
            return Ok(instance);
        }
        if self.machine.is_none() && storage::load_machine()?.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No registered machine available for Pi registration",
            ));
        }
        Ok(instance)
    }
    pub fn disconnect_pi(&self, _instance: &InstanceRecord) -> io::Result<()> {
        Ok(())
    }
}
pub fn default_registration() -> RadiusRegistration {
    RadiusRegistration {
        heartbeat_interval_ms: 30_000,
        expires_in_ms: 60_000,
    }
}
