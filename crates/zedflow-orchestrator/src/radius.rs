use crate::{
    config, storage,
    types::{InstanceRecord, MachineRecord, RadiusRegistration},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{Duration, sleep},
};

pub const DEFAULT_RADIUS_URL: &str = "https://radius.pi.dev/";
const NOT_FOUND_RETRY_THRESHOLD: u32 = 3;

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
    if let Ok(text) = std::fs::read_to_string(config::auth_path()) {
        if let Some(token) = serde_json::from_str::<AuthFile>(&text)
            .map_err(io::Error::other)?
            .radius
            .filter(|credential| credential.kind == "oauth")
            .and_then(|credential| credential.access)
            .filter(|token| !token.is_empty())
        {
            return Ok(token);
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
    1_000u64
        .saturating_mul(2u64.saturating_pow(failures.saturating_sub(1)))
        .min(30_000)
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

#[derive(Deserialize)]
struct Registration {
    id: String,
    #[serde(flatten)]
    registration: RadiusRegistration,
}
#[derive(Debug)]
struct HttpError {
    status: reqwest::StatusCode,
    message: String,
}
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}: {}", self.status, self.message)
    }
}
impl std::error::Error for HttpError {}
async fn post<T: for<'a> Deserialize<'a>, B: Serialize>(
    client: &reqwest::Client,
    path: &str,
    body: &B,
) -> Result<T, HttpError> {
    let url = format!("{}{}", radius_orchestrator_base_url(), path);
    let response = client
        .post(url)
        .bearer_auth(radius_access_token().map_err(|e| HttpError {
            status: reqwest::StatusCode::UNAUTHORIZED,
            message: e.to_string(),
        })?)
        .json(body)
        .send()
        .await
        .map_err(|e| HttpError {
            status: reqwest::StatusCode::BAD_GATEWAY,
            message: e.to_string(),
        })?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(HttpError {
            status,
            message: text,
        });
    }
    serde_json::from_str(&text).map_err(|e| HttpError {
        status,
        message: e.to_string(),
    })
}
async fn maybe_post<B: Serialize>(
    client: &reqwest::Client,
    path: &str,
    body: &B,
) -> Result<(), HttpError> {
    let url = format!("{}{}", radius_orchestrator_base_url(), path);
    let response = client
        .post(url)
        .bearer_auth(radius_access_token().map_err(|e| HttpError {
            status: reqwest::StatusCode::UNAUTHORIZED,
            message: e.to_string(),
        })?)
        .json(body)
        .send()
        .await
        .map_err(|e| HttpError {
            status: reqwest::StatusCode::BAD_GATEWAY,
            message: e.to_string(),
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    Err(HttpError {
        status,
        message: response.text().await.unwrap_or_default(),
    })
}

type Recovery = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
type PiRecovery =
    Arc<dyn Fn(InstanceRecord) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[derive(Default)]
struct State {
    machine: Option<MachineRecord>,
    machine_task: Option<JoinHandle<()>>,
    pi_tasks: HashMap<String, JoinHandle<()>>,
    recovery: Option<Recovery>,
    pi_recovery: Option<PiRecovery>,
}
#[derive(Clone)]
pub struct RadiusPresence {
    client: reqwest::Client,
    state: Arc<Mutex<State>>,
}
impl Default for RadiusPresence {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            state: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl RadiusPresence {
    pub async fn set_recovery<F, Fut>(&self, recovery: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.state.lock().await.recovery = Some(Arc::new(move || Box::pin(recovery())));
    }

    async fn recover_pis(&self) {
        let recovery = self.state.lock().await.recovery.clone();
        if let Some(recovery) = recovery {
            recovery().await;
        }
    }

    pub async fn set_pi_recovery<F, Fut>(&self, recovery: F)
    where
        F: Fn(InstanceRecord) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.state.lock().await.pi_recovery =
            Some(Arc::new(move |instance| Box::pin(recovery(instance))));
    }

    async fn recover_pi(&self, instance: InstanceRecord) {
        let recovery = self.state.lock().await.pi_recovery.clone();
        if let Some(recovery) = recovery {
            recovery(instance).await;
        }
    }

    pub async fn start(&self, label: Option<String>) -> io::Result<Option<MachineRecord>> {
        self.register_machine(label, true).await
    }

    async fn register_machine(
        &self,
        label: Option<String>,
        abort_existing_heartbeat: bool,
    ) -> io::Result<Option<MachineRecord>> {
        if !is_radius_enabled() {
            return Ok(None);
        }
        let existing = self
            .state
            .lock()
            .await
            .machine
            .clone()
            .or(storage::load_machine()?);
        let registration: Registration = post(
            &self.client,
            "machines/register",
            &serde_json::json!({
                "machineId": existing.as_ref().map(|machine| &machine.id), "label": label,
                "hostname": env::var("HOSTNAME").unwrap_or_default(), "platform": env::consts::OS,
                "arch": env::consts::ARCH, "version": config::VERSION,
                "capabilities": { "spawn": true, "relay": false, "iroh": false }
            }),
        )
        .await
        .map_err(error)?;
        let machine = MachineRecord {
            id: registration.id,
            created_at: existing.map(|m| m.created_at).unwrap_or_else(now),
            last_seen_at: Some(now()),
            label,
        };
        storage::save_machine(&machine)?;
        let mut state = self.state.lock().await;
        if abort_existing_heartbeat {
            if let Some(task) = state.machine_task.take() {
                task.abort();
            }
        }
        state.machine = Some(machine.clone());
        state.machine_task =
            Some(self.spawn_machine_heartbeat(registration.registration.heartbeat_interval_ms));
        Ok(Some(machine))
    }
    pub async fn stop(&self) -> io::Result<()> {
        let (machine, tasks) = {
            let mut state = self.state.lock().await;
            let mut tasks = state
                .pi_tasks
                .drain()
                .map(|(_, task)| task)
                .collect::<Vec<_>>();
            if let Some(task) = state.machine_task.take() {
                tasks.push(task);
            }
            (state.machine.take(), tasks)
        };
        for task in tasks {
            task.abort();
        }
        if let Some(machine) = machine {
            if is_radius_enabled() {
                match maybe_post(
                    &self.client,
                    &format!("machines/{}/disconnect", machine.id),
                    &serde_json::json!({}),
                )
                .await
                {
                    Ok(())
                    | Err(HttpError {
                        status: reqwest::StatusCode::NOT_FOUND,
                        ..
                    }) => (),
                    Err(e) => return Err(error(e)),
                }
            }
        }
        Ok(())
    }
    pub async fn register_pi(&self, instance: InstanceRecord) -> io::Result<InstanceRecord> {
        if !is_radius_enabled() {
            return Ok(instance);
        }
        let machine = self
            .state
            .lock()
            .await
            .machine
            .clone()
            .or(storage::load_machine()?)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "No registered machine available for Pi registration",
                )
            })?;
        let registration: Registration = post(&self.client, "pis/register", &serde_json::json!({
            "machineId": machine.id, "label": instance.label, "cwd": instance.cwd,
            "hostname": env::var("HOSTNAME").unwrap_or_default(), "pid": std::process::id(), "transport": "local-rpc",
            "capabilities": { "rpc": true, "relay": false, "iroh": false }, "sessionId": instance.session_id
        })).await.map_err(error)?;
        let mut result = instance;
        result.radius_pi_id = Some(registration.id.clone());
        let mut state = self.state.lock().await;
        if let Some(task) = state.pi_tasks.remove(&result.id) {
            task.abort();
        }
        state.pi_tasks.insert(
            result.id.clone(),
            self.spawn_pi_heartbeat(
                result.id.clone(),
                registration.id,
                registration.registration.heartbeat_interval_ms,
            ),
        );
        Ok(result)
    }
    pub async fn disconnect_pi(&self, instance: &InstanceRecord) -> io::Result<()> {
        if let Some(task) = self.state.lock().await.pi_tasks.remove(&instance.id) {
            task.abort();
        }
        let Some(id) = &instance.radius_pi_id else {
            return Ok(());
        };
        if !is_radius_enabled() {
            return Ok(());
        }
        match maybe_post(
            &self.client,
            &format!("pis/{id}/disconnect"),
            &serde_json::json!({}),
        )
        .await
        {
            Ok(())
            | Err(HttpError {
                status: reqwest::StatusCode::NOT_FOUND,
                ..
            }) => Ok(()),
            Err(e) => Err(error(e)),
        }
    }
    fn spawn_machine_heartbeat(&self, interval_ms: u64) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            let mut not_found = 0;
            let mut failures = 0;
            loop {
                sleep(Duration::from_millis(if failures == 0 {
                    interval_ms
                } else {
                    compute_backoff_delay_ms(failures)
                }))
                .await;
                let machine = this.state.lock().await.machine.clone();
                let Some(machine) = machine else { return };
                match maybe_post(&this.client, &format!("machines/{}/heartbeat", machine.id), &serde_json::json!({ "cwd": config::orchestrator_dir(), "socketPath": config::socket_path() })).await { Ok(()) => { not_found = 0; failures = 0; }, Err(e) if e.status == reqwest::StatusCode::NOT_FOUND => { not_found += 1; failures = 0; if not_found >= NOT_FOUND_RETRY_THRESHOLD { let label = machine.label; if this.register_machine(label, false).await.is_ok() { this.recover_pis().await; } return; } }, Err(_) => failures += 1 }
            }
        })
    }
    fn spawn_pi_heartbeat(
        &self,
        instance_id: String,
        radius_id: String,
        interval_ms: u64,
    ) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            let mut not_found = 0;
            let mut failures = 0;
            let radius_id = radius_id;
            loop {
                sleep(Duration::from_millis(if failures == 0 {
                    interval_ms
                } else {
                    compute_backoff_delay_ms(failures)
                }))
                .await;
                match maybe_post(
                    &this.client,
                    &format!("pis/{radius_id}/heartbeat"),
                    &serde_json::json!({}),
                )
                .await
                {
                    Ok(()) => {
                        not_found = 0;
                        failures = 0;
                    }
                    Err(e) if e.status == reqwest::StatusCode::NOT_FOUND => {
                        not_found += 1;
                        failures = 0;
                        if not_found >= NOT_FOUND_RETRY_THRESHOLD {
                            let Some(instance) = storage::get_instance(&instance_id).ok().flatten()
                            else {
                                return;
                            };
                            match this.register_pi(instance).await {
                                Ok(updated) => {
                                    let _ = storage::upsert_instance(&updated);
                                    this.recover_pi(updated).await;
                                    return;
                                }
                                Err(_) => failures = 1,
                            };
                        }
                    }
                    Err(_) => failures += 1,
                }
            }
        })
    }
}
pub fn default_registration() -> RadiusRegistration {
    RadiusRegistration {
        heartbeat_interval_ms: 30_000,
        expires_in_ms: 60_000,
    }
}
