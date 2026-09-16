//! Bounded acquisition of exact, explicitly declared local package closures.
use crate::source_acceptance::FilePrecondition;
use anyhow::{Context, Result};
use std::path::Path;
use zf_flows::package::PackageSnapshot;

#[derive(Debug)]
pub struct CapturedPackage {
    pub snapshot: PackageSnapshot,
    pub preconditions: Vec<FilePrecondition>,
}

/// Capture a directory containing `flow.json`, without compilation or network I/O.
///
/// # Errors
/// Rejects undeclared or missing files, symlinks in any path component, special
/// files, dependency cycles, resource excess, and changes observed during capture.
pub async fn capture(path: &Path) -> Result<PackageSnapshot> {
    Ok(capture_with_preconditions(path).await?.snapshot)
}

/// Capture the closure and SHA-256 preconditions for every manifest and file.
/// Preconditions refer to absolute source paths, including external dependencies.
///
/// # Errors
/// Has the same acquisition requirements as [`capture`]. Safe descriptor-relative
/// acquisition is currently supported on Unix; other platforms fail explicitly.
pub async fn capture_with_preconditions(path: &Path) -> Result<CapturedPackage> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || capture_sync(&path))
        .await
        .context("package acquisition worker failed")?
}

pub(super) fn capture_sync(path: &Path) -> Result<CapturedPackage> {
    #[cfg(unix)]
    {
        unix::capture(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        anyhow::bail!("safe package acquisition requires Unix")
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use anyhow::{bail, ensure};
    use nix::{
        dir::Dir,
        fcntl::{AtFlags, OFlag, openat},
        sys::stat::{Mode, SFlag, fstatat},
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs::{File, Metadata},
        io::Read,
        os::unix::fs::MetadataExt,
        path::{Component, PathBuf},
    };
    use zf_flows::package::{
        FlowPackageManifest, MANIFEST_FILE, MAX_PACKAGE_FILE_BYTES, MAX_PACKAGE_FILES,
        MAX_SNAPSHOT_BYTES, MAX_SNAPSHOT_PACKAGES, PackageNode,
    };

    const MAX_DEPTH: usize = 64;
    const MAX_ENTRIES: usize = 32768;
    const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Stamp {
        device: u64,
        inode: u64,
        length: u64,
        regular: bool,
        modified: (i64, i64),
        changed: (i64, i64),
    }
    impl From<&Metadata> for Stamp {
        fn from(meta: &Metadata) -> Self {
            Self {
                device: meta.dev(),
                inode: meta.ino(),
                length: meta.len(),
                regular: meta.is_file(),
                modified: (meta.mtime(), meta.mtime_nsec()),
                changed: (meta.ctime(), meta.ctime_nsec()),
            }
        }
    }
    struct CapturedFile {
        stamp: Stamp,
        hash: String,
        limit: usize,
    }
    struct PackageRecord {
        revision: String,
        descendants: BTreeSet<PathBuf>,
        declared: BTreeSet<String>,
        inventory: BTreeMap<String, Stamp>,
    }
    #[derive(Default)]
    struct State {
        active: BTreeSet<PathBuf>,
        records: BTreeMap<PathBuf, PackageRecord>,
        nodes: BTreeMap<String, PackageNode>,
        files: BTreeMap<PathBuf, CapturedFile>,
        bytes: usize,
        entries: usize,
    }

    fn directory_flags() -> OFlag {
        OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_DIRECTORY
    }

    // Open every component before reducing `..`; a symlink followed by `..`
    // must fail, even if lexical normalization would otherwise conceal it.
    fn directory(path: &Path) -> Result<(PathBuf, File)> {
        let absolute = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        ensure!(
            absolute.components().count() <= MAX_DEPTH * 2,
            "package path is too deep"
        );
        let mut file = File::from(nix::fcntl::open(
            Path::new("/"),
            directory_flags(),
            Mode::empty(),
        )?);
        let mut normalized = PathBuf::from("/");
        for part in absolute.components() {
            let component = match part {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => Path::new(name),
                Component::ParentDir => Path::new(".."),
                Component::Prefix(_) => bail!("unsupported package path prefix"),
            };
            file = File::from(
                openat(&file, component, directory_flags(), Mode::empty()).with_context(|| {
                    format!("unsafe or absent package directory: {}", path.display())
                })?,
            );
            if part == Component::ParentDir {
                normalized.pop();
            } else {
                normalized.push(component);
            }
        }
        Ok((normalized, file))
    }

    // libc's device/inode types vary between Unix platforms; MetadataExt
    // exposes them as u64, using the same integer representation conversion.
    #[allow(clippy::unnecessary_cast)]
    fn same_inode(meta: &Metadata, stat: &nix::sys::stat::FileStat) -> bool {
        meta.dev() == stat.st_dev as u64 && meta.ino() == stat.st_ino as u64
    }

    fn open_regular(path: &Path) -> Result<File> {
        let (_, parent) = directory(path.parent().context("package file has no parent")?)?;
        let name = Path::new(path.file_name().context("package file has no name")?);
        let stat = fstatat(&parent, name, AtFlags::AT_SYMLINK_NOFOLLOW)?;
        ensure!(
            SFlag::from_bits_truncate(stat.st_mode) == SFlag::S_IFREG,
            "package inventory must contain regular files: {}",
            path.display()
        );
        // NONBLOCK also prevents a concurrently substituted FIFO from blocking.
        let file = File::from(openat(
            &parent,
            name,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK,
            Mode::empty(),
        )?);
        let meta = file.metadata()?;
        ensure!(
            meta.is_file() && same_inode(&meta, &stat),
            "package file changed while opening: {}",
            path.display()
        );
        Ok(file)
    }

    impl State {
        fn read(&mut self, path: &Path, limit: usize) -> Result<Vec<u8>> {
            let mut file = open_regular(path)?;
            let before = file.metadata()?;
            let size = usize::try_from(before.len()).context("package file size overflow")?;
            ensure!(
                size <= limit,
                "package file exceeds byte limit: {}",
                path.display()
            );
            self.bytes = self
                .bytes
                .checked_add(size)
                .context("package closure size overflow")?;
            ensure!(
                self.bytes <= MAX_SNAPSHOT_BYTES,
                "package closure exceeds byte limit"
            );
            ensure!(
                self.files.len() < MAX_ENTRIES,
                "too many package closure files"
            );
            let mut bytes = vec![0; size];
            file.read_exact(&mut bytes)?;
            ensure!(
                file.read(&mut [0u8; 1])? == 0
                    && Stamp::from(&before) == Stamp::from(&file.metadata()?),
                "package file changed during capture: {}",
                path.display()
            );
            let captured = CapturedFile {
                stamp: Stamp::from(&before),
                hash: crate::context_store::hash(&bytes),
                limit,
            };
            if let Some(previous) = self.files.insert(path.to_owned(), captured) {
                ensure!(
                    previous.hash == crate::context_store::hash(&bytes),
                    "shared package file changed during capture"
                );
            }
            Ok(bytes)
        }

        fn package(&mut self, input: &Path, depth: usize) -> Result<String> {
            ensure!(depth <= MAX_DEPTH, "package dependency depth exceeds limit");
            let (root, _) = directory(input)?;
            ensure!(
                !self.active.contains(&root),
                "cyclic local package dependency: {}",
                root.display()
            );
            if let Some(record) = self.records.get(&root) {
                return Ok(record.revision.clone());
            }
            ensure!(
                self.records.len() + self.active.len() < MAX_SNAPSHOT_PACKAGES,
                "too many package directories"
            );
            self.active.insert(root.clone());
            let manifest_source =
                String::from_utf8(self.read(&root.join(MANIFEST_FILE), MAX_MANIFEST_BYTES)?)
                    .context("package manifest is not UTF-8")?;
            let manifest = FlowPackageManifest::parse(&manifest_source)?;
            let mut dependencies = BTreeMap::new();
            let mut descendants = BTreeSet::new();
            for (alias, dependency) in &manifest.dependencies {
                let dependency_path = root.join(&dependency.path);
                let revision = self.package(&dependency_path, depth + 1)?;
                let (resolved, _) = directory(&dependency_path)?;
                let record = self
                    .records
                    .get(&resolved)
                    .context("dependency changed during acquisition")?;
                descendants.extend(record.descendants.iter().cloned());
                descendants.insert(resolved);
                dependencies.insert(alias.clone(), revision);
            }
            let declared: BTreeSet<_> = manifest
                .files
                .iter()
                .cloned()
                .chain([MANIFEST_FILE.to_owned()])
                .collect();
            let inventory = inventory(&root, &declared, &descendants, &mut self.entries)?;
            let mut files = BTreeMap::new();
            for name in &manifest.files {
                files.insert(
                    name.clone(),
                    self.read(&root.join(name), MAX_PACKAGE_FILE_BYTES)?,
                );
            }
            let node = PackageNode {
                manifest_source,
                files,
                dependencies,
            };
            let revision = node.revision();
            if let Some(previous) = self.nodes.get(&revision) {
                ensure!(previous == &node, "conflicting package revision contents");
            } else {
                self.nodes.insert(revision.clone(), node);
            }
            self.active.remove(&root);
            self.records.insert(
                root,
                PackageRecord {
                    revision: revision.clone(),
                    descendants,
                    declared,
                    inventory,
                },
            );
            Ok(revision)
        }

        fn verify_inventory(&self) -> Result<()> {
            let mut entries = 0;
            for (root, record) in &self.records {
                ensure!(
                    inventory(root, &record.declared, &record.descendants, &mut entries)?
                        == record.inventory,
                    "package inventory changed during capture: {}",
                    root.display()
                );
            }
            Ok(())
        }

        fn verify(&self) -> Result<()> {
            self.verify_inventory()?;
            // Reopen through checked components and compare exact bytes after all
            // nodes were captured. This is an optimistic snapshot, not a writer lock.
            for (path, captured) in &self.files {
                let mut file = open_regular(path)?;
                let before = file.metadata()?;
                ensure!(
                    Stamp::from(&before) == captured.stamp && before.len() <= captured.limit as u64,
                    "package file changed during capture: {}",
                    path.display()
                );
                use sha2::{Digest, Sha256};
                let mut hash = Sha256::new();
                let mut remaining = before.len();
                let mut buffer = [0u8; 8192];
                while remaining > 0 {
                    let count = usize::try_from(remaining.min(buffer.len() as u64))?;
                    file.read_exact(&mut buffer[..count])?;
                    hash.update(&buffer[..count]);
                    remaining -= count as u64;
                }
                ensure!(
                    file.read(&mut [0u8; 1])? == 0
                        && Stamp::from(&file.metadata()?) == captured.stamp
                        && format!("{:x}", hash.finalize()) == captured.hash,
                    "package file changed during capture: {}",
                    path.display()
                );
            }
            self.verify_inventory()
        }
    }

    fn inventory(
        root: &Path,
        declared: &BTreeSet<String>,
        descendants: &BTreeSet<PathBuf>,
        entries: &mut usize,
    ) -> Result<BTreeMap<String, Stamp>> {
        let (_, directory) = directory(root)?;
        let mut result = BTreeMap::new();
        result.insert(String::new(), Stamp::from(&directory.metadata()?));
        walk(
            root,
            &directory,
            "",
            declared,
            descendants,
            entries,
            &mut result,
            0,
        )?;
        let present: BTreeSet<_> = result
            .iter()
            .filter(|(_, stamp)| stamp.regular)
            .map(|(name, _)| name.clone())
            .collect();
        ensure!(
            &present == declared,
            "package inventory differs from declared files: {}",
            root.display()
        );
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        root: &Path,
        parent: &File,
        relative: &str,
        declared: &BTreeSet<String>,
        descendants: &BTreeSet<PathBuf>,
        entries: &mut usize,
        result: &mut BTreeMap<String, Stamp>,
        depth: usize,
    ) -> Result<()> {
        ensure!(
            depth <= MAX_DEPTH,
            "package inventory directory depth exceeds limit"
        );
        let mut directory = Dir::openat(parent, ".", directory_flags(), Mode::empty())?;
        for entry in directory.iter() {
            let entry = entry?;
            let name = entry
                .file_name()
                .to_str()
                .context("package inventory path is not UTF-8")?;
            if matches!(name, "." | "..") {
                continue;
            }
            *entries += 1;
            ensure!(
                *entries <= MAX_ENTRIES,
                "too many package inventory entries"
            );
            let name = if relative.is_empty() {
                name.to_owned()
            } else {
                format!("{relative}/{name}")
            };
            let path = root.join(&name);
            let local = Path::new(entry.file_name().to_str()?);
            let stat = fstatat(parent, local, AtFlags::AT_SYMLINK_NOFOLLOW)?;
            let kind = SFlag::from_bits_truncate(stat.st_mode);
            if kind == SFlag::S_IFDIR {
                let child = File::from(openat(parent, local, directory_flags(), Mode::empty())?);
                let meta = child.metadata()?;
                ensure!(
                    same_inode(&meta, &stat),
                    "package directory changed during capture"
                );
                result.insert(name.clone(), Stamp::from(&meta));
                if descendants.contains(&path) {
                    continue;
                }
                let prefix = format!("{name}/");
                ensure!(
                    declared.iter().any(|file| file.starts_with(&prefix))
                        || descendants
                            .iter()
                            .any(|dependency| dependency.starts_with(&path)),
                    "undeclared package directory: {}",
                    path.display()
                );
                walk(
                    root,
                    &child,
                    &name,
                    declared,
                    descendants,
                    entries,
                    result,
                    depth + 1,
                )?;
            } else {
                ensure!(
                    kind == SFlag::S_IFREG,
                    "package inventory contains symlink or special file: {}",
                    path.display()
                );
                ensure!(
                    declared.contains(&name),
                    "undeclared package file: {}",
                    path.display()
                );
                let file = open_regular(&path)?;
                result.insert(name, Stamp::from(&file.metadata()?));
            }
            ensure!(
                result.len() <= MAX_ENTRIES && declared.len() <= MAX_PACKAGE_FILES + 1,
                "package inventory exceeds limit"
            );
        }
        Ok(())
    }

    pub(super) fn capture(path: &Path) -> Result<CapturedPackage> {
        let mut state = State::default();
        let root = state.package(path, 0)?;
        state.verify()?;
        let snapshot = PackageSnapshot {
            root,
            packages: state.nodes,
        };
        snapshot.validate()?;
        let preconditions = state
            .files
            .into_iter()
            .map(|(path, file)| FilePrecondition {
                path,
                hash: file.hash,
            })
            .collect();
        Ok(CapturedPackage {
            snapshot,
            preconditions,
        })
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        fn acquired() -> (tempfile::TempDir, State) {
            let temp = tempfile::tempdir().unwrap();
            std::fs::write(temp.path().join("flow.json"),
                r#"{"formatVersion":1,"id":"root","name":"Root","entry":"flow.rs","files":["flow.rs","other.rs"]}"#).unwrap();
            std::fs::write(temp.path().join("flow.rs"), b"root").unwrap();
            std::fs::write(temp.path().join("other.rs"), b"old").unwrap();
            let mut state = State::default();
            state.package(temp.path(), 0).unwrap();
            (temp, state)
        }

        #[test]
        fn revalidation_rejects_secondary_file_changes() {
            let (temp, state) = acquired();
            std::fs::write(temp.path().join("other.rs"), b"new").unwrap();
            assert!(state.verify().is_err());
        }

        #[test]
        fn revalidation_rejects_added_inventory() {
            let (temp, state) = acquired();
            std::fs::write(temp.path().join("extra.rs"), b"extra").unwrap();
            assert!(state.verify().is_err());
        }

        #[test]
        fn revalidation_rejects_directory_replaced_with_symlink() {
            let (temp, state) = acquired();
            let parent = tempfile::tempdir().unwrap();
            let moved = parent.path().join("moved");
            std::fs::rename(temp.path(), &moved).unwrap();
            std::os::unix::fs::symlink(&moved, temp.path()).unwrap();
            assert!(state.verify().is_err());
            std::fs::remove_file(temp.path()).unwrap();
        }
    }
}
