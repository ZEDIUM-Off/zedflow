use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::provenance::{ExtensionSource, ProvenanceReceipt, digest_file, digest_tree};

/// Copies a development source into an empty staging directory. Symlinks and
/// build/VCS output are refused, so Cargo never builds unreviewed artifacts.
pub fn stage_source(source: &Path, staging: &Path) -> io::Result<()> {
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    fs::create_dir_all(staging)?;
    copy_tree(source, staging)
}

pub fn build_source(staging: &Path) -> Result<(), String> {
    let manifest = staging.join("Cargo.toml");
    if !manifest.is_file() {
        return Err("extension source has no Cargo.toml".into());
    }
    let fetch = Command::new("cargo")
        .args(["fetch", "--locked", "--manifest-path"])
        .arg(&manifest)
        .status()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    if !fetch.success() {
        return Err("extension Cargo fetch failed".into());
    }
    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--offline",
            "--release",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", staging.join("target"))
        .status()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("extension Cargo build failed".into())
    }
}

pub fn store_artifact(store: &Path, artifact: &Path) -> io::Result<PathBuf> {
    let digest = digest_file(artifact)?;
    let destination =
        store
            .join(&digest)
            .join(artifact.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "artifact has no filename")
            })?);
    if !destination.exists() {
        fs::create_dir_all(destination.parent().expect("parent"))?;
        fs::copy(artifact, &destination)?;
    }
    Ok(destination)
}

pub fn build_and_store(
    source: &ExtensionSource,
    source_dir: &Path,
    staging: &Path,
    artifact: &Path,
    store: &Path,
    previous: Option<String>,
) -> Result<(PathBuf, ProvenanceReceipt), String> {
    stage_source(source_dir, staging).map_err(|error| error.to_string())?;
    let source_sha256 = digest_tree(staging).map_err(|error| error.to_string())?;
    build_source(staging)?;
    if artifact.is_absolute()
        || artifact
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("extension artifact must be relative to the staged source".into());
    }
    let staged_artifact = staging.join(artifact);
    let target = fs::canonicalize(staging.join("target")).map_err(|error| error.to_string())?;
    let actual = fs::canonicalize(&staged_artifact).map_err(|error| error.to_string())?;
    if !actual.starts_with(&target) {
        return Err("extension artifact was not produced by the local Cargo build".into());
    }
    let artifact_sha256 = digest_file(&actual).map_err(|error| error.to_string())?;
    let stored = store_artifact(store, &actual).map_err(|error| error.to_string())?;
    Ok((
        stored,
        ProvenanceReceipt {
            source: source.canonical(),
            source_sha256,
            artifact_sha256,
            previous_artifact_sha256: previous,
        },
    ))
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some(".git" | "target")) {
            continue;
        }
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "extension source contains symlink",
            ));
        }
        let target = destination.join(&name);
        if ty.is_dir() {
            fs::create_dir(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artifact_store_is_content_addressed() {
        let root = std::env::temp_dir().join(format!("zedflow-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let artifact = root.join("plugin.so");
        fs::write(&artifact, b"plugin").unwrap();
        let stored = store_artifact(&root.join("store"), &artifact).unwrap();
        assert_eq!(fs::read(stored).unwrap(), b"plugin");
        let _ = fs::remove_dir_all(root);
    }
}
