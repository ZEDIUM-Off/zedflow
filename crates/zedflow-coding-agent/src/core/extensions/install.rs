use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use super::provenance::{ExtensionSource, ProvenanceReceipt, digest_file, digest_tree};

/// Obtains a source tree only after the caller has approved the project.
/// Registry and Git sources are copied or checked out as source; no downloaded
/// executable is ever accepted.
pub fn acquire_source(
    source: &ExtensionSource,
    cache: &Path,
    trusted: bool,
) -> Result<PathBuf, String> {
    if !trusted {
        return Err("extension source installation requires a trusted project".into());
    }
    match source {
        ExtensionSource::Path(path) => fs::canonicalize(path)
            .map_err(|error| format!("failed to resolve extension path: {error}"))
            .and_then(|path| {
                if path.is_dir() {
                    Ok(path)
                } else {
                    Err("extension path source must be a directory".into())
                }
            }),
        ExtensionSource::Github {
            owner,
            repo,
            commit,
            package,
        } => acquire_github(owner, repo, commit, package.as_deref(), cache),
        ExtensionSource::Crate { name, version } => acquire_crate(name, version, cache),
    }
}

fn acquire_github(
    owner: &str,
    repo: &str,
    commit: &str,
    package: Option<&str>,
    cache: &Path,
) -> Result<PathBuf, String> {
    let destination = cache.join("github").join(owner).join(repo).join(commit);
    if !destination.exists() {
        fs::create_dir_all(destination.parent().expect("github cache parent"))
            .map_err(|error| error.to_string())?;
        let status = Command::new("git")
            .args([
                "clone",
                "--no-checkout",
                &format!("https://github.com/{owner}/{repo}.git"),
            ])
            .arg(&destination)
            .status()
            .map_err(|error| format!("failed to start git: {error}"))?;
        if !status.success() {
            return Err("Git source retrieval failed".into());
        }
        run_git_in(&destination, &["fetch", "--depth", "1", "origin", commit])?;
        run_git_in(&destination, &["checkout", "--detach", commit])?;
    }
    let actual = git_output(&destination, &["rev-parse", "HEAD"])?;
    if actual.trim() != commit {
        return Err("GitHub extension checkout does not match its pinned commit".into());
    }
    let root = package.map_or_else(|| destination.clone(), |package| destination.join(package));
    if root.is_dir() {
        Ok(root)
    } else {
        Err("GitHub extension package directory does not exist".into())
    }
}

fn acquire_crate(name: &str, version: &str, cache: &Path) -> Result<PathBuf, String> {
    let work = cache.join("crates").join(name).join(version);
    let manifest = work.join("Cargo.toml");
    if !manifest.exists() {
        fs::create_dir_all(work.join("src")).map_err(|error| error.to_string())?;
        fs::write(
            &manifest,
            format!(
                "[package]\nname = \"zedflow-extension-fetch\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nextension = {{ package = \"{name}\", version = \"={version}\" }}\n"
            ),
        )
        .map_err(|error| error.to_string())?;
        fs::write(work.join("src/lib.rs"), "").map_err(|error| error.to_string())?;
        run_cargo(&["generate-lockfile", "--manifest-path"], &manifest)?;
        run_cargo(&["fetch", "--locked", "--manifest-path"], &manifest)?;
    }
    let output = cargo_output(
        &[
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ],
        &manifest,
    )?;
    let metadata: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| format!("invalid Cargo metadata: {error}"))?;
    metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages.iter().find_map(|package| {
                (package["name"].as_str() == Some(name)
                    && package["version"].as_str() == Some(version)
                    && package["source"]
                        .as_str()
                        .is_some_and(|source| source.starts_with("registry+")))
                .then(|| package["manifest_path"].as_str())
                .flatten()
            })
        })
        .map(PathBuf::from)
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
        .ok_or_else(|| "Cargo did not resolve the requested registry crate source".into())
}

fn run_cargo(args: &[&str], manifest: &Path) -> Result<(), String> {
    let mut command = Command::new("cargo");
    let status = command
        .args(args)
        .arg(manifest)
        .status()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "Cargo source retrieval failed".into())
}

fn cargo_output(args: &[&str], manifest: &Path) -> Result<String, String> {
    let output = Command::new("cargo")
        .args(args)
        .arg(manifest)
        .output()
        .map_err(|error| format!("failed to start cargo: {error}"))?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
        .ok_or_else(|| "Cargo metadata failed".into())
}

fn run_git_in(cwd: &Path, args: &[&str]) -> Result<(), String> {
    run_git_command(Command::new("git").args(args).current_dir(cwd))
}

fn run_git_command(command: &mut Command) -> Result<(), String> {
    command
        .status()
        .map_err(|error| format!("failed to start git: {error}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "Git source retrieval failed".into())
}

fn git_output(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("failed to start git: {error}"))?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
        .ok_or_else(|| "Git verification failed".into())
}

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
    fn path_sources_require_trust_and_are_not_copied_as_artifacts() {
        let root = std::env::temp_dir().join(format!("zedflow-install-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        let input = ExtensionSource::Path(source.clone());
        assert!(acquire_source(&input, &root.join("cache"), false).is_err());
        assert_eq!(
            acquire_source(&input, &root.join("cache"), true).unwrap(),
            fs::canonicalize(source).unwrap()
        );
        let _ = fs::remove_dir_all(root);
    }
}
