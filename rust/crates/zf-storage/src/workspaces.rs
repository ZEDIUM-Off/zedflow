//! Workspace identities and browsing belong to the daemon's filesystem.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub open: bool,
}

pub fn path_id(path: &Path) -> String {
    format!("{:x}", Sha256::digest(path.as_os_str().as_encoded_bytes()))
}

pub async fn initialize(db: &SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS workspaces(id TEXT PRIMARY KEY, document TEXT NOT NULL)",
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list(db: &SqlitePool) -> Result<Vec<Workspace>> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT document FROM workspaces ORDER BY rowid")
        .fetch_all(db)
        .await?;
    rows.iter()
        .map(|row| Ok(serde_json::from_str(row)?))
        .collect()
}

#[derive(Debug)]
pub struct WorkspaceNotFound;
impl std::fmt::Display for WorkspaceNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Workspace introuvable")
    }
}
impl std::error::Error for WorkspaceNotFound {}

pub async fn get(db: &SqlitePool, id: &str) -> Result<Workspace> {
    let row: String = sqlx::query_scalar("SELECT document FROM workspaces WHERE id=?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(WorkspaceNotFound)?;
    Ok(serde_json::from_str(&row)?)
}

pub async fn save(db: &SqlitePool, workspace: &Workspace) -> Result<()> {
    sqlx::query("INSERT INTO workspaces(id,document) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET document=excluded.document")
        .bind(&workspace.id).bind(serde_json::to_string(workspace)?).execute(db).await?;
    Ok(())
}

pub async fn open(db: &SqlitePool, path: &Path) -> Result<Workspace> {
    let path = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("Impossible d’ouvrir le dossier {}", path.display()))?;
    ensure!(
        tokio::fs::metadata(&path).await?.is_dir(),
        "Le workspace doit être un dossier"
    );
    // Opening also checks directory access, rather than accepting an unusable cwd.
    let _entries = tokio::fs::read_dir(&path)
        .await
        .context("Lecture du workspace refusée")?;
    ensure_metadata(&path).await?;
    let id = path_id(&path);
    let mut workspace = list(db)
        .await?
        .into_iter()
        .find(|w| w.id == id)
        .unwrap_or_else(|| Workspace {
            id,
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            path,
            open: true,
        });
    workspace.open = true;
    save(db, &workspace).await?;
    Ok(workspace)
}

/// Exported sessions and flows are shareable; live runtime data stays local.
pub async fn ensure_metadata(workspace: &Path) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let directory = workspace.join(".zedflow");
    crate::session_archive::owned_directory(&directory).await?;
    let path = directory.join(".gitignore");
    let block = "# Zedflow local runtime\n/*.db\n/*.db-*\n/runs/\n/builds/\n/session-downloads/\n/.session-imports/\n/sessions/.export-*/\n/sessions/.previous-*/\n";
    let context_block =
        "# Zedflow context authoring\n/context/.catalog.lock\n/context/.context-*.tmp\n";
    let sources_block = "# Zedflow source transactions\n/.sources.lock\n/.source-import.json\n/.source-import-*/\n/.source-journal-*.tmp\n/.source-acceptance.json\n/source-history/\n/previews/\n**/.accepted-*.tmp\n**/.zedflow-*.tmp\n";
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Le .gitignore Zedflow doit être un fichier ordinaire"
            );
            let existing = tokio::fs::read_to_string(&path).await?;
            let mut additions = String::new();
            if !existing.contains("# Zedflow local runtime") {
                additions.push_str(block);
            }
            if !existing.contains("# Zedflow context authoring") {
                additions.push_str(context_block);
            }
            if !existing.contains("# Zedflow source transactions") {
                additions.push_str(sources_block);
            }
            if !additions.is_empty() {
                let mut file = tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .await?;
                if !existing.is_empty() && !existing.ends_with('\n') {
                    file.write_all(b"\n").await?;
                }
                file.write_all(additions.as_bytes()).await?;
                file.sync_all().await?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await?;
            file.write_all(block.as_bytes()).await?;
            file.write_all(context_block.as_bytes()).await?;
            file.write_all(sources_block.as_bytes()).await?;
            file.sync_all().await?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub async fn browse(path: &Path, home: &Path, show_hidden: bool) -> Result<Value> {
    let path = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("Dossier introuvable ou inaccessible : {}", path.display()))?;
    ensure!(
        tokio::fs::metadata(&path).await?.is_dir(),
        "Ce chemin n’est pas un dossier"
    );
    let mut dir = tokio::fs::read_dir(&path)
        .await
        .context("Accès au dossier refusé")?;
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        match tokio::fs::metadata(entry.path()).await {
            Ok(metadata) if metadata.is_dir() => {
                entries.push(json!({"name":name,"path":entry.path(),"directory":true}))
            }
            Ok(_) => {}
            Err(error) => diagnostics.push(format!("{name} : {error}")),
        }
    }
    entries.sort_by_key(|entry| entry["name"].as_str().unwrap_or_default().to_lowercase());
    Ok(
        json!({"path":path,"parent":path.parent(),"home":home,"entries":entries,"diagnostics":diagnostics}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn repository_ignore_policy_is_unchanged_and_keeps_only_explicit_artifacts_versionable() {
        let directory = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(["-c", "core.excludesFile="])
                .arg("-C")
                .arg(directory.path())
                .args(args)
                .output()
                .unwrap()
        };
        assert!(git(&["init", "-q"]).status.success());
        std::fs::create_dir(directory.path().join(".zedflow")).unwrap();
        let policy = include_bytes!("../../../../.zedflow/.gitignore");
        let policy_path = directory.path().join(".zedflow/.gitignore");
        std::fs::write(&policy_path, policy).unwrap();
        std::fs::write(
            directory.path().join(".gitignore"),
            include_bytes!("../../../../.gitignore"),
        )
        .unwrap();

        for attempt in 1..=2 {
            ensure_metadata(directory.path()).await.unwrap();
            assert_eq!(
                std::fs::read(&policy_path).unwrap(),
                policy,
                "Opening the repository must preserve its policy on attempt {attempt}"
            );
        }

        for path in [
            ".zedflow/.gitignore",
            ".zedflow/flow/review/src/lib.rs",
            ".zedflow/flows/review.rs",
            ".zedflow/context/review.rs",
            ".zedflow/context/libraries/terms.rs",
            ".zedflow/types/terms.rs",
            ".zedflow/examples/review.rs",
            ".zedflow/bridges/docs.rs",
            ".zedflow/sessions/shared/journal.jsonl",
        ] {
            assert_eq!(
                git(&["check-ignore", "--no-index", path]).status.code(),
                Some(1),
                "Chosen artifact must remain versionable: {path}"
            );
        }
        for path in [
            ".zedflow/private-note",
            ".zedflow/local.db",
            ".zedflow/local.db-wal",
            ".zedflow/runs/run.json",
            ".zedflow/builds/run/output",
            ".zedflow/session-downloads/export.zip",
            ".zedflow/.session-imports/draft/journal.jsonl",
            ".zedflow/sessions/.export-draft/journal.jsonl",
            ".zedflow/sessions/.previous-draft/journal.jsonl",
            ".zedflow/.sources.lock",
            ".zedflow/.source-import.json",
            ".zedflow/.source-import-draft/0.rs",
            ".zedflow/.source-journal-draft.tmp",
            ".zedflow/.source-acceptance.json",
            ".zedflow/source-history/lineage.json",
            ".zedflow/previews/id/workspace/file",
            ".zedflow/context/.catalog.lock",
            ".zedflow/context/.context-draft.tmp",
            ".zedflow/flow/review/.catalog.lock",
            ".zedflow/flow/review/.zedflow-draft.tmp",
            ".zedflow/types/.accepted-draft.tmp",
        ] {
            assert!(
                git(&["check-ignore", "--no-index", path]).status.success(),
                "Runtime and temporary files must stay local: {path}"
            );
        }
    }

    #[tokio::test]
    async fn source_metadata_ignores_transactions_but_keeps_explicit_artifacts_versionable() {
        let directory = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(directory.path())
                .args(args)
                .output()
                .unwrap()
        };
        assert!(git(&["init", "-q"]).status.success());
        std::fs::create_dir(directory.path().join(".zedflow")).unwrap();
        std::fs::write(
            directory.path().join(".zedflow/.gitignore"),
            "# Custom rule\n/private-note",
        )
        .unwrap();
        ensure_metadata(directory.path()).await.unwrap();
        let first = std::fs::read_to_string(directory.path().join(".zedflow/.gitignore")).unwrap();
        ensure_metadata(directory.path()).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(directory.path().join(".zedflow/.gitignore")).unwrap(),
            first
        );
        assert!(first.starts_with("# Custom rule\n/private-note\n"));
        for path in [
            ".zedflow/context/review.rs",
            ".zedflow/context/libraries/terms.rs",
            ".zedflow/types/terms.rs",
            ".zedflow/bridges/docs.rs",
            ".zedflow/flows/review.rs",
            ".zedflow/sessions/shared/journal.jsonl",
        ] {
            assert_eq!(
                git(&["check-ignore", "--no-index", path]).status.code(),
                Some(1),
                "Chosen artifact must remain versionable: {path}"
            );
        }
        for path in [
            ".zedflow/.sources.lock",
            ".zedflow/.source-acceptance.json",
            ".zedflow/.source-import.json",
            ".zedflow/source-history/lineage.json",
            ".zedflow/.source-import-draft/0.rs",
            ".zedflow/context/.accepted-replace-id.tmp",
            ".zedflow/types/.accepted-id.tmp",
            ".zedflow/previews/id/workspace/file",
        ] {
            assert!(
                git(&["check-ignore", "--no-index", path]).status.success(),
                "Runtime transaction must stay local: {path}"
            );
        }
    }

    #[tokio::test]
    async fn canonical_identity_reopens_without_losing_name_and_browses_directories() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(dir.path().join("visible"))
            .await
            .unwrap();
        tokio::fs::create_dir(dir.path().join(".hidden"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("file"), "hello")
            .await
            .unwrap();
        let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
        initialize(&db).await.unwrap();
        let mut workspace = open(&db, dir.path()).await.unwrap();
        workspace.open = false;
        workspace.name = "Project A".into();
        save(&db, &workspace).await.unwrap();
        let reopened = open(&db, &dir.path().join("visible/..")).await.unwrap();
        assert_eq!(workspace.id, reopened.id);
        assert_eq!(reopened.name, "Project A");
        assert!(reopened.open);
        assert_eq!(list(&db).await.unwrap().len(), 1);
        let listing = browse(dir.path(), dir.path(), false).await.unwrap();
        assert_eq!(listing["entries"].as_array().unwrap().len(), 1);
        assert_eq!(listing["entries"][0]["name"], "visible");
        let with_hidden = browse(dir.path(), dir.path(), true).await.unwrap();
        assert_eq!(with_hidden["entries"].as_array().unwrap().len(), 3);
        assert!(
            with_hidden["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["name"] == ".zedflow")
        );
        assert!(dir.path().join(".zedflow/.gitignore").is_file());
        assert!(
            browse(&dir.path().join("file"), dir.path(), false)
                .await
                .is_err()
        );
    }
}
