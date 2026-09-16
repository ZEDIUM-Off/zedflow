//! Explicit template creation; existing files, including invalid sources, survive.
use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Kind {
    Flow,
    Bridge,
    Context,
}

pub(crate) fn create(workspace: &Path, kind: Kind, id: &str) -> Result<PathBuf> {
    ensure!(
        zf_context::context::valid_id(id),
        "identifiant attendu : 1 à 160 lettres ASCII, chiffres, tirets ou underscores"
    );
    let workspace = fs::canonicalize(workspace).context("workspace introuvable")?;
    let namespace = directory(&workspace, ".zedflow")?;
    let parent = directory(
        &namespace,
        match kind {
            Kind::Flow => "flow",
            Kind::Bridge => "bridges",
            Kind::Context => "context",
        },
    )?;
    match kind {
        Kind::Flow => {
            let path = parent.join(id);
            // create_dir is exclusive: no existing directory, file or symlink is reused.
            fs::create_dir(&path).with_context(|| {
                format!(
                    "création refusée : {} existe déjà ou est inaccessible",
                    path.display()
                )
            })?;
            for (name, source) in [
                ("flow.json", include_str!("../templates/flow/flow.json")),
                ("flow.rs", include_str!("../templates/flow/flow.rs")),
                ("README.md", include_str!("../templates/flow/README.md")),
            ] {
                write_new(&path.join(name), &source.replace("{{id}}", id))?;
            }
            Ok(path)
        }
        Kind::Bridge | Kind::Context => {
            let source = match kind {
                Kind::Bridge => include_str!("../templates/bridge.rs"),
                Kind::Context => include_str!("../templates/context.rs"),
                Kind::Flow => unreachable!(),
            };
            let path = parent.join(format!("{id}.rs"));
            write_new(&path, &source.replace("{{id}}", id))?;
            Ok(path)
        }
    }
}

fn directory(parent: &Path, name: &str) -> Result<PathBuf> {
    let path = parent.join(name);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("création de {}", path.display())),
    }
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_dir() && !metadata.is_symlink(),
        "répertoire réel requis : {}",
        path.display()
    );
    Ok(path)
}

fn write_new(path: &Path, source: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("création sans écrasement de {}", path.display()))?;
    file.write_all(source.as_bytes())
        .with_context(|| format!("écriture de {}", path.display()))?;
    file.sync_all()?;
    Ok(())
}
