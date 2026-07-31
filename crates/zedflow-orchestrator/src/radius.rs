use crate::{
    config, storage,
    types::{InstanceRecord, MachineRecord, RadiusRegistration},
};
use reqwest::{Url, blocking::Client};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    env, io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_RADIUS_URL: &str = "https://radius.pi.dev/";
const NOT_FOUND_RETRY_THRESHOLD: u32 = 3;
const HEARTBEAT_BACKOFF_BASE_MS: u64 = 1_000;
const HEARTBEAT_BACKOFF_MAX_MS: u64 = 30_000;

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
    let auth_path = env::var_os("PI_CONFIG_DIR")
        .map(Into::into)
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(|home| std::path::PathBuf::from(home).join(".pi/agent"))
                .unwrap_or_else(|| ".pi/agent".into())
        })
        .join("auth.json");
    if let Ok(text) = std::fs::read_to_string(auth_path) {
        let auth: AuthFile = serde_json::from_str(&text).map_err(io::Error::other)?;
        if let Some(access) = auth
            .radius
            .filter(|credential| credential.kind == "oauth")
            .and_then(|credential| credential.access)
            .filter(|token| !token.is_empty())
        {
            return Ok(access);
        }
    }
    env::var("PI_RADIUS_API_KEY")
        .ok()
        .filter(|key| !key.is_empty())
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
    HEARTBEAT_BACKOFF_BASE_MS
        .saturating_mul(2u64.saturating_pow(failures.saturating_sub(1)))
        .min(HEARTBEAT_BACKOFF_MAX_MS)
}

#[derive(Debug)]
struct RadiusError {
    status: Option<u16>,
    message: String,
}
impl std::fmt::Display for RadiusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for RadiusError {}
type Result<T> = std::result::Result<T, RadiusError>;

fn post<T: DeserializeOwned>(path: &str, body: impl Serialize) -> Result<T> {
    let url = Url::parse(&radius_orchestrator_base_url())
        .and_then(|base| base.join(path))
        .map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?;
    let response = Client::new()
        .post(url)
        .bearer_auth(radius_access_token().map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?)
        .json(&body)
        .send()
        .map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(RadiusError {
            status: Some(status.as_u16()),
            message: format!(
                "Radius request failed: {status} {}",
                response.text().unwrap_or_default()
            ),
        });
    }
    response.json().map_err(|error| RadiusError {
        status: None,
        message: error.to_string(),
    })
}
fn maybe_post(path: &str, body: impl Serialize) -> Result<()> {
    let url = Url::parse(&radius_orchestrator_base_url())
        .and_then(|base| base.join(path))
        .map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?;
    let response = Client::new()
        .post(url)
        .bearer_auth(radius_access_token().map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?)
        .json(&body)
        .send()
        .map_err(|error| RadiusError {
            status: None,
            message: error.to_string(),
        })?;
    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        Err(RadiusError {
            status: Some(status.as_u16()),
            message: format!(
                "Radius request failed: {status} {}",
                response.text().unwrap_or_default()
            ),
        })
    }
}
fn is_not_found(error: &RadiusError) -> bool {
    error.status == Some(404)
}
fn io_error(error: RadiusError) -> io::Error {
    io::Error::other(error)
}
fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[derive(Deserialize)]
struct Registration {
    id: String,
    #[serde(flatten)]
    registration: RadiusRegistration,
}
#[derive(Clone)]
struct PiHeartbeat {
    interval_ms: u64,
    radius_pi_id: String,
    not_found: u32,
    failures: u32,
    instance: InstanceRecord,
}
#[derive(Default)]
struct State {
    machine: Option<MachineRecord>,
    machine_interval_ms: u64,
    machine_not_found: u32,
    machine_failures: u32,
    pis: HashMap<String, PiHeartbeat>,
    stopped: bool,
}
#[derive(Clone, Default)]
pub struct RadiusPresence(Arc<Mutex<State>>);

impl RadiusPresence {
    pub fn start(&self, label: Option<String>) -> io::Result<Option<MachineRecord>> {
        if !is_radius_enabled() {
            return Ok(None);
        }
        let registered = self.register_machine(label)?;
        self.schedule_machine(registered.registration.heartbeat_interval_ms);
        Ok(self
            .0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?
            .machine
            .clone())
    }
    pub fn stop(&self) -> io::Result<()> {
        let machine = {
            let mut state = self
                .0
                .lock()
                .map_err(|_| io::Error::other("radius lock poisoned"))?;
            state.stopped = true;
            state.pis.clear();
            state.machine.clone()
        };
        if let Some(machine) = machine {
            if is_radius_enabled() {
                match maybe_post(
                    &format!("machines/{}/disconnect", machine.id),
                    serde_json::json!({}),
                ) {
                    Ok(()) => (),
                    Err(error) if is_not_found(&error) => (),
                    Err(error) => return Err(io_error(error)),
                }
            }
        }
        Ok(())
    }
    pub fn register_pi(&self, instance: InstanceRecord) -> io::Result<InstanceRecord> {
        if !is_radius_enabled() {
            return Ok(instance);
        }
        let machine = self
            .0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?
            .machine
            .clone()
            .or(storage::load_machine()?)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "No registered machine available for Pi registration",
                )
            })?;
        let registered: Registration = post("pis/register", serde_json::json!({ "machineId": machine.id, "label": instance.label, "cwd": instance.cwd, "hostname": env::var("HOSTNAME").unwrap_or_default(), "pid": std::process::id(), "transport": "local-rpc", "capabilities": { "rpc": true, "relay": false, "iroh": false }, "sessionId": instance.session_id })).map_err(io_error)?;
        let mut instance = instance;
        instance.radius_pi_id = Some(registered.id.clone());
        self.0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?
            .pis
            .insert(
                instance.id.clone(),
                PiHeartbeat {
                    interval_ms: registered.registration.heartbeat_interval_ms,
                    radius_pi_id: registered.id,
                    not_found: 0,
                    failures: 0,
                    instance: instance.clone(),
                },
            );
        self.schedule_pi(&instance.id, registered.registration.heartbeat_interval_ms);
        Ok(instance)
    }
    pub fn disconnect_pi(&self, instance: &InstanceRecord) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?
            .pis
            .remove(&instance.id);
        if !is_radius_enabled() {
            return Ok(());
        }
        if let Some(id) = &instance.radius_pi_id {
            match maybe_post(&format!("pis/{id}/disconnect"), serde_json::json!({})) {
                Ok(()) => (),
                Err(error) if is_not_found(&error) => (),
                Err(error) => return Err(io_error(error)),
            }
        }
        Ok(())
    }
    fn register_machine(&self, label: Option<String>) -> io::Result<Registration> {
        let old = self
            .0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?
            .machine
            .clone()
            .or(storage::load_machine()?);
        let registered: Registration = post("machines/register", serde_json::json!({ "machineId": old.as_ref().map(|machine| &machine.id), "label": label, "hostname": env::var("HOSTNAME").unwrap_or_default(), "platform": env::consts::OS, "arch": env::consts::ARCH, "version": config::VERSION, "capabilities": { "spawn": true, "relay": false, "iroh": false } })).map_err(io_error)?;
        let machine = MachineRecord {
            id: registered.id.clone(),
            created_at: old
                .map(|machine| machine.created_at)
                .unwrap_or_else(timestamp),
            last_seen_at: Some(timestamp()),
            label,
        };
        storage::save_machine(&machine)?;
        let mut state = self
            .0
            .lock()
            .map_err(|_| io::Error::other("radius lock poisoned"))?;
        state.machine = Some(machine);
        state.machine_interval_ms = registered.registration.heartbeat_interval_ms;
        state.machine_not_found = 0;
        state.machine_failures = 0;
        state.stopped = false;
        Ok(registered)
    }
    fn schedule_machine(&self, delay: u64) {
        let this = self.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay));
            let _ = this.heartbeat_machine();
        });
    }
    fn schedule_pi(&self, id: &str, delay: u64) {
        let this = self.clone();
        let id = id.to_owned();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay));
            let _ = this.heartbeat_pi(&id);
        });
    }
    fn heartbeat_machine(&self) -> io::Result<()> {
        let machine = {
            let state = self
                .0
                .lock()
                .map_err(|_| io::Error::other("radius lock poisoned"))?;
            if state.stopped {
                return Ok(());
            }
            state.machine.clone()
        };
        let Some(machine) = machine else {
            return Ok(());
        };
        match maybe_post(
            &format!("machines/{}/heartbeat", machine.id),
            serde_json::json!({ "cwd": config::orchestrator_dir(), "socketPath": config::socket_path() }),
        ) {
            Ok(()) => {
                let interval = {
                    let mut s = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?;
                    s.machine_not_found = 0;
                    s.machine_failures = 0;
                    s.machine_interval_ms
                };
                self.schedule_machine(interval);
            }
            Err(error) if is_not_found(&error) => {
                let recover = {
                    let mut s = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?;
                    s.machine_not_found += 1;
                    s.machine_failures = 0;
                    s.machine_not_found >= NOT_FOUND_RETRY_THRESHOLD
                };
                if recover {
                    let (label, instances) = {
                        let state = self
                            .0
                            .lock()
                            .map_err(|_| io::Error::other("radius lock poisoned"))?;
                        (
                            state
                                .machine
                                .as_ref()
                                .and_then(|machine| machine.label.clone()),
                            state
                                .pis
                                .values()
                                .map(|heartbeat| heartbeat.instance.clone())
                                .collect::<Vec<_>>(),
                        )
                    };
                    let registration = self.register_machine(label)?;
                    self.schedule_machine(registration.registration.heartbeat_interval_ms);
                    for instance in instances {
                        let registered = self.register_pi(instance)?;
                        storage::upsert_instance(&registered)?;
                    }
                } else {
                    let delay = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?
                        .machine_interval_ms;
                    self.schedule_machine(delay);
                }
            }
            Err(_) => {
                let delay = {
                    let mut s = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?;
                    s.machine_failures += 1;
                    compute_backoff_delay_ms(s.machine_failures)
                };
                self.schedule_machine(delay);
            }
        }
        Ok(())
    }
    pub fn heartbeat_pi(&self, id: &str) -> io::Result<()> {
        let state = {
            let state = self
                .0
                .lock()
                .map_err(|_| io::Error::other("radius lock poisoned"))?;
            if state.stopped {
                return Ok(());
            }
            state.pis.get(id).cloned()
        };
        let Some(beat) = state else {
            return Ok(());
        };
        match maybe_post(
            &format!("pis/{}/heartbeat", beat.radius_pi_id),
            serde_json::json!({}),
        ) {
            Ok(()) => {
                let mut s = self
                    .0
                    .lock()
                    .map_err(|_| io::Error::other("radius lock poisoned"))?;
                if let Some(b) = s.pis.get_mut(id) {
                    b.not_found = 0;
                    b.failures = 0;
                    self.schedule_pi(id, b.interval_ms);
                }
            }
            Err(error) if is_not_found(&error) => {
                let recover = {
                    let mut s = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?;
                    let b = s.pis.get_mut(id).expect("heartbeat state exists");
                    b.not_found += 1;
                    b.failures = 0;
                    b.not_found >= NOT_FOUND_RETRY_THRESHOLD
                };
                if recover {
                    let recovered = self.register_pi(beat.instance)?;
                    storage::upsert_instance(&recovered)?;
                } else {
                    self.schedule_pi(id, beat.interval_ms);
                }
            }
            Err(_) => {
                let delay = {
                    let mut s = self
                        .0
                        .lock()
                        .map_err(|_| io::Error::other("radius lock poisoned"))?;
                    let b = s.pis.get_mut(id).expect("heartbeat state exists");
                    b.failures += 1;
                    compute_backoff_delay_ms(b.failures)
                };
                self.schedule_pi(id, delay);
            }
        }
        Ok(())
    }
}
pub fn default_registration() -> RadiusRegistration {
    RadiusRegistration {
        heartbeat_interval_ms: 30_000,
        expires_in_ms: 60_000,
    }
}
