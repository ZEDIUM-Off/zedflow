use crate::{
    radius::RadiusPresence,
    rpc_process::RpcProcessInstance,
    storage,
    types::{InstanceRecord, InstanceStatus},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn refreshes_metadata(command: &Value) -> bool {
    matches!(
        command.get("type").and_then(Value::as_str),
        Some("new_session" | "switch_session" | "fork" | "clone" | "set_session_name" | "prompt")
    )
}
fn session_metadata(response: &Value) -> Option<(String, Option<String>)> {
    (response.get("success")?.as_bool()? && response.get("command")?.as_str()? == "get_state")
        .then(|| {
            let data = response.get("data")?;
            Some((
                data.get("sessionId")?.as_str()?.to_owned(),
                data.get("sessionFile")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            ))
        })
        .flatten()
}

pub struct OrchestratorSupervisor {
    live: Arc<Mutex<HashMap<String, RpcProcessInstance>>>,
    records: Arc<Mutex<()>>,
    radius: RadiusPresence,
}
impl Default for OrchestratorSupervisor {
    fn default() -> Self {
        Self::new()
    }
}
impl OrchestratorSupervisor {
    pub fn new() -> Self {
        Self {
            live: Arc::new(Mutex::new(HashMap::new())),
            records: Arc::new(Mutex::new(())),
            radius: RadiusPresence::default(),
        }
    }
    pub async fn start_radius(&self) -> io::Result<()> {
        self.radius.start(None).await.map(|_| ())
    }
    pub async fn recover_after_restart(&mut self) -> io::Result<()> {
        let stamp = now();
        let instances = storage::load_instances()?
            .into_iter()
            .map(|mut instance| {
                if matches!(
                    instance.status,
                    InstanceStatus::Online | InstanceStatus::Starting
                ) {
                    instance.status = InstanceStatus::Stopped;
                }
                instance.last_seen_at = Some(stamp.clone());
                instance
            })
            .collect::<Vec<_>>();
        for instance in &instances {
            self.radius.disconnect_pi(instance).await?;
        }
        storage::save_instances(&instances)
    }
    pub fn list_instances(&self) -> io::Result<Vec<InstanceRecord>> {
        let _records = self.records.lock().unwrap();
        storage::load_instances()
    }
    pub fn get_instance(&self, id: &str) -> io::Result<Option<InstanceRecord>> {
        let _records = self.records.lock().unwrap();
        storage::get_instance(id)
    }
    fn sync_session_metadata(&self, id: &str, process: &RpcProcessInstance) -> io::Result<()> {
        let response = process.send(serde_json::json!({"type": "get_state"}))?;
        let Some((session_id, session_file)) = session_metadata(&response) else {
            return Ok(());
        };
        let _records = self.records.lock().unwrap();
        let Some(mut record) = storage::get_instance(id)? else {
            return Ok(());
        };
        record.session_id = Some(session_id);
        record.session_file = session_file;
        record.last_seen_at = Some(now());
        storage::upsert_instance(&record)
    }
    fn bind_exit(&self, id: String, process: &RpcProcessInstance) {
        let live = self.live.clone();
        let records = self.records.clone();
        let radius = self.radius.clone();
        let handle = tokio::runtime::Handle::current();
        process.on_exit(move || {
            if live.lock().unwrap().remove(&id).is_none() {
                return;
            }
            let _records = records.lock().unwrap();
            let Ok(Some(mut record)) = storage::get_instance(&id) else {
                return;
            };
            if matches!(
                record.status,
                InstanceStatus::Stopping | InstanceStatus::Stopped
            ) {
                return;
            }
            record.status = InstanceStatus::Error;
            record.last_seen_at = Some(now());
            let _ = storage::upsert_instance(&record);
            let radius = radius.clone();
            handle.spawn(async move {
                let _ = radius.disconnect_pi(&record).await;
            });
        });
    }
    pub async fn spawn_instance(
        &mut self,
        cwd: String,
        label: Option<String>,
    ) -> io::Result<InstanceRecord> {
        let stamp = now();
        let mut record = InstanceRecord {
            id: Uuid::new_v4().to_string(),
            status: InstanceStatus::Starting,
            cwd: cwd.clone(),
            created_at: stamp.clone(),
            last_seen_at: Some(stamp),
            label,
            session_id: None,
            session_file: None,
            radius_pi_id: None,
        };
        storage::upsert_instance(&record)?;
        match RpcProcessInstance::new(&cwd) {
            Ok(process) => {
                self.live
                    .lock()
                    .unwrap()
                    .insert(record.id.clone(), process.clone());
                self.bind_exit(record.id.clone(), &process);
                self.sync_session_metadata(&record.id, &process)?;
                record = storage::get_instance(&record.id)?.expect("spawned record exists");
                record = self.radius.register_pi(record).await?;
                record.status = InstanceStatus::Online;
                record.last_seen_at = Some(now());
                storage::upsert_instance(&record)?;
                Ok(record)
            }
            Err(error) => {
                record.status = InstanceStatus::Stopped;
                storage::upsert_instance(&record)?;
                Err(error)
            }
        }
    }
    pub async fn stop_instance(&mut self, id: &str) -> io::Result<Option<InstanceRecord>> {
        let Some(mut record) = storage::get_instance(id)? else {
            return Ok(None);
        };
        if let Some(process) = self.live.lock().unwrap().remove(id) {
            record.status = InstanceStatus::Stopping;
            storage::upsert_instance(&record)?;
            process.dispose()?;
        }
        self.radius.disconnect_pi(&record).await?;
        record.status = InstanceStatus::Stopped;
        record.last_seen_at = Some(now());
        storage::remove_instance(id)?;
        Ok(Some(record))
    }
    pub fn handle_rpc(&mut self, id: &str, command: Value) -> io::Result<Option<Value>> {
        let Some(process) = self.live.lock().unwrap().get(id).cloned() else {
            return Ok(None);
        };
        let response = process.send(command.clone())?;
        if refreshes_metadata(&command) {
            self.sync_session_metadata(id, &process)?;
        }
        Ok(Some(response))
    }
    pub fn open_rpc_stream(
        &self,
        id: &str,
        events: tokio::sync::mpsc::UnboundedSender<Value>,
        ui_requests: tokio::sync::mpsc::UnboundedSender<Value>,
    ) -> Option<(RpcProcessInstance, u64)> {
        let process = self.live.lock().unwrap().get(id)?.clone();
        let subscriber = process.subscribe(events, ui_requests);
        Some((process, subscriber))
    }
    pub async fn shutdown(&mut self) -> io::Result<()> {
        let ids = self
            .live
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for id in ids {
            self.stop_instance(&id).await?;
        }
        self.radius.stop().await
    }
}
