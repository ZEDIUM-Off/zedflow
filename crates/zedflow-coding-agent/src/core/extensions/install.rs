use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

use super::provenance::{ExtensionSource, ProvenanceReceipt, digest_file, digest_tree};

/// Copies a development source into an empty staging directory. Symlinks and
/// build/VCS output are refused, so Cargo never builds unreviewed artifacts.
pub fn stage_source(source: &Path, staging: &Path) -> io::Result<()> {
    if fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "extension source must not be a symlink",
        ));
    }
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "extension source must be a directory",
        ));
    }
    if staging.exists() {
        fs::remove_dir_all(staging)?;
    }
    fs::create_dir_all(staging)?;
    copy_tree(source, staging)
}

/// Materializes only source code for a pinned extension source. GitHub sources
/// are checked out at their declared commit; crates are copied from Cargo's
/// resolved registry source, never from an installed binary.
pub fn materialize_source(source: &ExtensionSource, destination: &Path) -> Result<PathBuf, String> {
    match source {
        ExtensionSource::Path(path) => {
            stage_source(path, destination).map_err(|error| error.to_string())?;
            Ok(destination.to_path_buf())
        }
        ExtensionSource::Github {
            owner,
            repo,
            commit,
            package,
        } => {
            if destination.exists() {
                fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
            }
            fs::create_dir_all(destination.parent().unwrap_or_else(|| Path::new(".")))
                .map_err(|error| error.to_string())?;
            run(
                "git",
                [
                    "clone".into(),
                    "--no-checkout".into(),
                    format!("https://github.com/{owner}/{repo}.git"),
                    destination.display().to_string(),
                ],
            )?;
            run(
                "git",
                [
                    "-C".into(),
                    destination.display().to_string(),
                    "checkout".into(),
                    "--detach".into(),
                    commit.clone(),
                ],
            )?;
            let head = Command::new("git")
                .args([
                    "-C",
                    &destination.display().to_string(),
                    "rev-parse",
                    "HEAD",
                ])
                .output()
                .map_err(|error| format!("failed to start git: {error}"))?;
            if !head.status.success()
                || !String::from_utf8_lossy(&head.stdout)
                    .trim()
                    .eq_ignore_ascii_case(commit)
            {
                return Err("GitHub checkout did not resolve to the requested commit".into());
            }
            Ok(package
                .as_ref()
                .map_or_else(|| destination.to_path_buf(), |path| destination.join(path)))
        }
        ExtensionSource::Crate { name, version } => materialize_crate(name, version, destination),
    }
}

/// Fetches a source, builds it locally, and stores only the local build output.
pub fn install_source(
    source: &ExtensionSource,
    source_work_dir: &Path,
    staging: &Path,
    artifact: &Path,
    store: &Path,
    previous: Option<String>,
) -> Result<(PathBuf, ProvenanceReceipt), String> {
    let source_dir = materialize_source(source, source_work_dir)?;
    build_and_store(source, &source_dir, staging, artifact, store, previous)
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

fn materialize_crate(name: &str, version: &str, destination: &Path) -> Result<PathBuf, String> {
    let fetch_dir = destination.with_extension("fetch");
    if fetch_dir.exists() {
        fs::remove_dir_all(&fetch_dir).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&fetch_dir).map_err(|error| error.to_string())?;
    let manifest = fetch_dir.join("Cargo.toml");
    let dependency = serde_json::to_string(name).expect("string serializes");
    fs::write(
        &manifest,
        format!(
            "[package]\nname = \"zedflow-extension-fetch\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency} = \"={version}\"\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    let manifest_arg = manifest.display().to_string();
    run(
        "cargo",
        [
            "generate-lockfile".into(),
            "--manifest-path".into(),
            manifest_arg.clone(),
        ],
    )?;
    run(
        "cargo",
        [
            "fetch".into(),
            "--locked".into(),
            "--manifest-path".into(),
            manifest_arg.clone(),
        ],
    )?;
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    if !output.status.success() {
        return Err("Cargo metadata failed while resolving extension crate".into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid Cargo metadata: {error}"))?;
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages.iter().find(|package| {
                package["name"].as_str() == Some(name)
                    && package["version"].as_str() == Some(version)
            })
        })
        .and_then(|package| package["manifest_path"].as_str())
        .ok_or("resolved extension crate source was not found")?;
    let source = Path::new(package)
        .parent()
        .ok_or("resolved extension crate manifest has no parent")?;
    stage_source(source, destination).map_err(|error| error.to_string())?;
    let _ = fs::remove_dir_all(fetch_dir);
    Ok(destination.to_path_buf())
}

fn run<I>(program: &str, args: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|error| format!("failed to start {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} command failed"))
    }
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
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "extension source contains unsupported file type",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "zedflow-install-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn path_sources_are_copied_without_vcs_or_build_output() {
        let root = temp_dir();
        let source = root.join("source");
        fs::create_dir_all(source.join(".git")).unwrap();
        fs::create_dir_all(source.join("target")).unwrap();
        fs::write(
            source.join("Cargo.toml"),
            "[package]\nname=\"x\"\nversion=\"0.1.0\"",
        )
        .unwrap();
        fs::write(source.join(".git/config"), "private").unwrap();
        fs::write(source.join("target/plugin.so"), "prebuilt").unwrap();
        let destination = root.join("destination");
        let actual = materialize_source(&ExtensionSource::Path(source), &destination).unwrap();
        assert_eq!(actual, destination);
        assert!(actual.join("Cargo.toml").is_file());
        assert!(!actual.join(".git").exists());
        assert!(!actual.join("target").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn artifact_store_is_content_addressed() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        let artifact = root.join("plugin.so");
        fs::write(&artifact, b"plugin").unwrap();
        let stored = store_artifact(&root.join("store"), &artifact).unwrap();
        assert_eq!(fs::read(stored).unwrap(), b"plugin");
        fs::remove_dir_all(root).unwrap();
    }
}
