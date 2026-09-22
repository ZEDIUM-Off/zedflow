//! Local release activation is delegated to the supervising process. Requests
//! contain release identities only, never paths or commands from the browser.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

pub fn build() -> Value {
    serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/build-info.json")))
        .expect("compiled build identity")
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
#[derive(Clone, Default)]
pub struct Config {
    pub web: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub release: Option<String>,
}
impl Config {
    pub fn managed(web: PathBuf) -> Self {
        Self {
            web: Some(web),
            root: std::env::var_os("ZEDFLOW_RELEASE_ROOT").map(PathBuf::from),
            release: std::env::var("ZEDFLOW_RELEASE_ID").ok(),
        }
    }
}
/// Only state/compatibility preconditions of an activation request are conflicts.
#[derive(Debug)]
pub(crate) struct RequestConflict(pub &'static str);
impl std::fmt::Display for RequestConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for RequestConflict {}

/// The selected release has no manifest; unrelated I/O failures are not absence.
#[derive(Debug)]
pub(crate) struct ReleaseNotFound;
impl std::fmt::Display for ReleaseNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Release absente")
    }
}
impl std::error::Error for ReleaseNotFound {}

pub struct Updates {
    pub config: Config,
    pub gate: Arc<RwLock<bool>>,
    pub requested: AtomicBool,
}
impl Updates {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            gate: Arc::new(RwLock::new(false)),
            requested: AtomicBool::new(false),
        }
    }
    pub async fn status(&self) -> Value {
        let client = match &self.config.web {
            Some(path) => read(&path.join("client-version.json")).await.ok(),
            None => None,
        };
        let mut result = json!({"daemon":build(),"client":client,"releaseId":self.config.release,"channel":"local","managed":false,"candidate":null,"previous":null,"operation":null,"maintenance":self.requested.load(Ordering::SeqCst)});
        if let Some(root) = &self.config.root {
            result["managed"] = json!(self.manager_alive().await.unwrap_or(false));
            result["operation"] = read(&root.join("status.json")).await.unwrap_or(Value::Null);
            let candidate_path = root.join("candidate.json");
            if tokio::fs::try_exists(&candidate_path).await.unwrap_or(true) {
                let candidate = async {
                    let pointer = read(&candidate_path).await?;
                    let id = pointer["releaseId"]
                        .as_str()
                        .context("Pointeur de release sans identité")?;
                    self.manifest(id).await
                }
                .await;
                match candidate {
                    Ok(value) => result["candidate"] = release_summary(&value),
                    Err(error) => {
                        result["catalogError"] =
                            json!(format!("Catalogue de mises à jour illisible : {error}"))
                    }
                }
            }
            if let Ok(current) = read(&root.join("current.json")).await
                && let Some(id) = current["previousReleaseId"].as_str()
            {
                result["previous"] = self
                    .manifest(id)
                    .await
                    .map(|value| release_summary(&value))
                    .unwrap_or(Value::Null);
            }
        }
        result
    }
    pub async fn manifest(&self, id: &str) -> Result<Value> {
        ensure!(valid_id(id), "Identifiant de release invalide");
        let root = self
            .config
            .root
            .as_ref()
            .context("Daemon lancé sans gestionnaire de mises à jour")?;
        let path = root.join("releases").join(id).join("manifest.json");
        match tokio::fs::symlink_metadata(&path).await {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ReleaseNotFound.into());
            }
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let value = read(&path).await?;
        ensure!(
            value["format"] == 1 && value["releaseId"] == id,
            "Manifeste de release invalide"
        );
        Ok(value)
    }
    async fn manager_alive(&self) -> Result<bool> {
        let Some(root) = &self.config.root else {
            return Ok(false);
        };
        let value = match read(&root.join("manager.json")).await {
            Ok(value) => value,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        Ok(value["ready"] == true
            && value["releaseId"].as_str() == self.config.release.as_deref()
            && value["updatedAt"]
                .as_u64()
                .is_some_and(|time| time <= now() && now() - time < 10_000))
    }
    pub async fn request(&self, target: &str, expected: &str) -> Result<Value> {
        ensure!(
            self.manager_alive().await?,
            RequestConflict("Le superviseur de mises à jour est indisponible")
        );
        ensure!(
            build()["buildId"] == expected,
            RequestConflict("Le daemon a changé ; actualisez les versions avant de réessayer")
        );
        ensure!(
            Some(target) != self.config.release.as_deref(),
            RequestConflict("Cette release est déjà active")
        );
        let manifest = self.manifest(target).await?;
        ensure!(
            manifest["daemon"]["storageEpoch"] == build()["storageEpoch"],
            RequestConflict("Cette mise à jour exige une migration hors ligne du stockage")
        );
        ensure!(
            manifest["daemon"]["protocol"] == manifest["client"]["protocol"],
            RequestConflict("Client et daemon de la release incompatibles")
        );
        ensure!(
            manifest["daemon"]["target"] == build()["target"],
            RequestConflict("Release construite pour une autre plateforme")
        );
        let request = json!({"id":uuid::Uuid::new_v4().to_string(),"target":target,"source":self.config.release,"expectedDaemonBuildId":expected,"createdAt":now()});
        atomic_json(
            &self.config.root.as_ref().unwrap().join("request.json"),
            &request,
        )
        .await?;
        self.requested.store(true, Ordering::SeqCst);
        Ok(request)
    }
}
pub async fn read(path: &Path) -> Result<Value> {
    let metadata = tokio::fs::symlink_metadata(path).await?;
    ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() <= 2 * 1024 * 1024,
        "Manifeste non ordinaire ou trop volumineux"
    );
    Ok(serde_json::from_slice(&tokio::fs::read(path).await?)?)
}
async fn atomic_json(path: &Path, value: &Value) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result: Result<()> = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(&serde_json::to_vec(value)?).await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&temporary, path).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        // The private staging file must not survive a failed write/rename.
        match tokio::fs::remove_file(&temporary).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    result?;
    // Persist the directory entry before acknowledging the activation request.
    let parent = path.parent().unwrap().to_owned();
    tokio::task::spawn_blocking(move || std::fs::File::open(parent)?.sync_all()).await??;
    Ok(())
}

fn release_summary(value: &Value) -> Value {
    json!({"releaseId":value["releaseId"],"createdAt":value["createdAt"],"daemon":value["daemon"],"client":value["client"]})
}
