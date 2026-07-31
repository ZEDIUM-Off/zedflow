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
pub struct OrchestratorSupervisor {
    live: HashMap<String, RpcProcessInstance>,
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
            live: HashMap::new(),
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
        storage::load_instances()
    }
    pub fn get_instance(&self, id: &str) -> io::Result<Option<InstanceRecord>> {
        storage::get_instance(id)
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
                record.status = InstanceStatus::Online;
                record.last_seen_at = Some(now());
                self.live.insert(record.id.clone(), process);
                record = self.radius.register_pi(record).await?;
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
        if let Some(mut process) = self.live.remove(id) {
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
        let Some(process) = self.live.get_mut(id) else {
            return Ok(None);
        };
        let response = process.send(command.clone())?;
        if refreshes_metadata(&command) {
            if let Some(record) = storage::get_instance(id)? {
                storage::upsert_instance(&record)?;
            }
        }
        Ok(Some(response))
    }
    pub async fn shutdown(&mut self) -> io::Result<()> {
        for id in self.live.keys().cloned().collect::<Vec<_>>() {
            self.stop_instance(&id).await?;
        }
        self.radius.stop().await
    }
}
