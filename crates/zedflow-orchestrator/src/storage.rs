use crate::{
    config,
    types::{InstanceRecord, MachineRecord},
};
use std::{fs, io, path::Path};

fn ensure_dir() -> io::Result<()> {
    fs::create_dir_all(config::orchestrator_dir())
}
fn load<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn save<T: serde::Serialize + ?Sized>(path: &Path, value: &T) -> io::Result<()> {
    ensure_dir()?;
    fs::write(
        path,
        serde_json::to_string_pretty(value).map_err(io::Error::other)?,
    )
}
pub fn load_machine() -> io::Result<Option<MachineRecord>> {
    load(&config::machine_path())
}
pub fn save_machine(machine: &MachineRecord) -> io::Result<()> {
    save(&config::machine_path(), machine)
}
pub fn delete_machine() -> io::Result<()> {
    match fs::remove_file(config::machine_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
pub fn load_instances() -> io::Result<Vec<InstanceRecord>> {
    Ok(load(&config::instances_path())?.unwrap_or_default())
}
pub fn save_instances(instances: &[InstanceRecord]) -> io::Result<()> {
    save(&config::instances_path(), instances)
}
pub fn get_instance(id: &str) -> io::Result<Option<InstanceRecord>> {
    Ok(load_instances()?
        .into_iter()
        .find(|instance| instance.id == id))
}
pub fn upsert_instance(instance: &InstanceRecord) -> io::Result<()> {
    let mut instances = load_instances()?;
    if let Some(old) = instances.iter_mut().find(|old| old.id == instance.id) {
        *old = instance.clone();
    } else {
        instances.push(instance.clone());
    }
    save_instances(&instances)
}
pub fn remove_instance(id: &str) -> io::Result<()> {
    save_instances(
        &load_instances()?
            .into_iter()
            .filter(|instance| instance.id != id)
            .collect::<Vec<_>>(),
    )
}
