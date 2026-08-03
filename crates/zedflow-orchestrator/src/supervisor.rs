use crate::{
    radius::{RadiusPresence, now},
    rpc_process::RpcProcessInstance,
    storage,
    types::{InstanceRecord, InstanceStatus},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
};
use uuid::Uuid;
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

#[derive(Clone)]
struct LiveInstance {
    process: RpcProcessInstance,
    record: InstanceRecord,
}

#[derive(Clone)]
pub struct RpcStreamHandle {
    process: RpcProcessInstance,
    subscriber: u64,
    id: String,
    live: Arc<Mutex<HashMap<String, LiveInstance>>>,
    records: Arc<Mutex<()>>,
}
impl RpcStreamHandle {
    pub fn handle_rpc(&self, command: Value) -> io::Result<Value> {
        let response = self.process.send(command.clone())?;
        if refreshes_metadata(&command) {
            sync_session_metadata(&self.id, &self.process, &self.live, &self.records)?;
        }
        Ok(response)
    }
    pub fn handle_ui_response(&self, response: &Value) -> io::Result<()> {
        self.process.handle_ui_response(response)
    }
    pub fn close(&self) {
        self.process.unsubscribe(self.subscriber);
    }
}

fn sync_session_metadata(
    id: &str,
    process: &RpcProcessInstance,
    live: &Arc<Mutex<HashMap<String, LiveInstance>>>,
    records: &Arc<Mutex<()>>,
) -> io::Result<()> {
    let response = process.send(serde_json::json!({"type": "get_state"}))?;
    let _records = records.lock().unwrap();
    let mut live = live.lock().unwrap();
    let Some(instance) = live.get_mut(id) else {
        return Ok(());
    };
    if let Some((session_id, session_file)) = session_metadata(&response) {
        instance.record.session_id = Some(session_id);
        instance.record.session_file = session_file;
    }
    instance.record.last_seen_at = Some(now());
    storage::upsert_instance(&instance.record)
}

pub struct OrchestratorSupervisor {
    live: Arc<Mutex<HashMap<String, LiveInstance>>>,
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
        let radius = self.radius.clone();
        let live = self.live.clone();
        let records = self.records.clone();
        self.radius
            .set_recovery(move || {
                let radius = radius.clone();
                let live = live.clone();
                let records = records.clone();
                async move {
                    let instances = live
                        .lock()
                        .unwrap()
                        .values()
                        .map(|instance| instance.record.clone())
                        .collect::<Vec<_>>();
                    for instance in instances {
                        if let Ok(updated) = radius.register_pi(instance).await {
                            if let Some(live_instance) = live.lock().unwrap().get_mut(&updated.id) {
                                live_instance.record = updated.clone();
                            }
                            let _records = records.lock().unwrap();
                            let _ = storage::upsert_instance(&updated);
                        }
                    }
                }
            })
            .await;
        let live = self.live.clone();
        let records = self.records.clone();
        self.radius
            .set_pi_recovery(move |updated| {
                let live = live.clone();
                let records = records.clone();
                async move {
                    if let Some(live_instance) = live.lock().unwrap().get_mut(&updated.id) {
                        live_instance.record = updated.clone();
                        let _records = records.lock().unwrap();
                        let _ = storage::upsert_instance(&updated);
                    }
                }
            })
            .await;
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
        if let Some(instance) = self.live.lock().unwrap().get(id) {
            return Ok(Some(instance.record.clone()));
        }
        let _records = self.records.lock().unwrap();
        storage::get_instance(id)
    }
    fn sync_session_metadata(&self, id: &str, process: &RpcProcessInstance) -> io::Result<()> {
        sync_session_metadata(id, process, &self.live, &self.records)
    }
    fn update_record(&self, record: InstanceRecord) -> io::Result<()> {
        if let Some(instance) = self.live.lock().unwrap().get_mut(&record.id) {
            instance.record = record.clone();
        }
        let _records = self.records.lock().unwrap();
        storage::upsert_instance(&record)
    }
    fn bind_exit(&self, id: String, process: &RpcProcessInstance) -> u64 {
        let live = self.live.clone();
        let records = self.records.clone();
        let radius = self.radius.clone();
        let handle = tokio::runtime::Handle::current();
        process.on_exit(move || {
            let Some(mut instance) = live.lock().unwrap().remove(&id) else {
                return;
            };
            if matches!(
                instance.record.status,
                InstanceStatus::Stopping | InstanceStatus::Stopped
            ) {
                return;
            }
            instance.record.status = InstanceStatus::Error;
            instance.record.last_seen_at = Some(now());
            let _records = records.lock().unwrap();
            let _ = storage::upsert_instance(&instance.record);
            let radius = radius.clone();
            let records = records.clone();
            let record = instance.record;
            handle.spawn(async move {
                if radius.disconnect_pi(&record).await.is_ok() {
                    let mut record = record;
                    record.radius_pi_id = None;
                    record.last_seen_at = Some(now());
                    let _records = records.lock().unwrap();
                    let _ = storage::upsert_instance(&record);
                }
            });
        })
    }
    async fn fail_spawn(
        &self,
        mut record: InstanceRecord,
        process: &RpcProcessInstance,
        exit_listener: u64,
    ) {
        process.remove_exit_listener(exit_listener);
        record.status = InstanceStatus::Error;
        record.last_seen_at = Some(now());
        let _ = self.update_record(record.clone());
        let _ = self.radius.disconnect_pi(&record).await;
        let _ = process.dispose();
        record.status = InstanceStatus::Stopped;
        record.last_seen_at = Some(now());
        let _ = self.update_record(record.clone());
        self.live.lock().unwrap().remove(&record.id);
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
        self.update_record(record.clone())?;
        match RpcProcessInstance::new(&cwd) {
            Ok(process) => {
                self.live.lock().unwrap().insert(
                    record.id.clone(),
                    LiveInstance {
                        process: process.clone(),
                        record: record.clone(),
                    },
                );
                let exit_listener = self.bind_exit(record.id.clone(), &process);
                if let Err(error) = self.sync_session_metadata(&record.id, &process) {
                    self.fail_spawn(record, &process, exit_listener).await;
                    return Err(error);
                }
                record = self
                    .get_instance(&record.id)?
                    .expect("spawned record exists");
                record = match self.radius.register_pi(record.clone()).await {
                    Ok(record) => record,
                    Err(error) => {
                        self.fail_spawn(record, &process, exit_listener).await;
                        return Err(error);
                    }
                };
                record.status = InstanceStatus::Online;
                record.last_seen_at = Some(now());
                self.update_record(record.clone())?;
                Ok(record)
            }
            Err(error) => {
                record.status = InstanceStatus::Stopped;
                self.update_record(record)?;
                Err(error)
            }
        }
    }
    pub async fn stop_instance(&mut self, id: &str) -> io::Result<Option<InstanceRecord>> {
        let Some(instance) = self.live.lock().unwrap().remove(id) else {
            return Ok(None);
        };
        let mut record = instance.record;
        record.status = InstanceStatus::Stopping;
        self.update_record(record.clone())?;
        let cleanup = async {
            instance.process.dispose()?;
            self.radius.disconnect_pi(&record).await
        }
        .await;
        storage::remove_instance(id)?;
        cleanup?;
        record.status = InstanceStatus::Stopped;
        record.last_seen_at = Some(now());
        Ok(Some(record))
    }
    pub fn handle_rpc(&mut self, id: &str, command: Value) -> io::Result<Option<Value>> {
        let Some(process) = self
            .live
            .lock()
            .unwrap()
            .get(id)
            .map(|instance| instance.process.clone())
        else {
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
    ) -> Option<RpcStreamHandle> {
        let process = self.live.lock().unwrap().get(id)?.process.clone();
        let subscriber = process.subscribe(events, ui_requests);
        Some(RpcStreamHandle {
            process,
            subscriber,
            id: id.to_owned(),
            live: self.live.clone(),
            records: self.records.clone(),
        })
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
