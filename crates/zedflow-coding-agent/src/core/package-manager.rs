//! Source-only native extension package operations.
//!
//! Pi's TypeScript package manager installs JavaScript packages.  Zedflow's
//! approved equivalent accepts only pinned Rust extension sources and delegates
//! compilation/provenance to the native extension installer.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::CONFIG_DIR_NAME,
    extensions::{ExtensionSource, NativeExtensionInstall, install_source},
};

pub const MODULE_PATH: &str = "core/package-manager.rs";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageScope {
    User,
    Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredPackage {
    pub source: String,
    pub scope: PackageScope,
    pub filtered: bool,
    pub installed_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceScope {
    User,
    Project,
    Temporary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathMetadata {
    pub source: String,
    pub scope: SourceScope,
    pub origin: ResourceOrigin,
    pub base_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceOrigin {
    Package,
    TopLevel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedResource {
    pub path: PathBuf,
    pub enabled: bool,
    pub metadata: PathMetadata,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedPaths {
    pub extensions: Vec<ResolvedResource>,
    pub skills: Vec<ResolvedResource>,
    pub prompts: Vec<ResolvedResource>,
    pub themes: Vec<ResolvedResource>,
}

/// Source and Cargo-produced artifact path needed for an installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePackageSpec {
    pub source: String,
    /// Path relative to the source package's Cargo target directory.
    pub artifact: PathBuf,
}

#[derive(Clone, Debug)]
pub struct DefaultPackageManager {
    cwd: PathBuf,
    agent_dir: PathBuf,
}

impl DefaultPackageManager {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, agent_dir: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            agent_dir: agent_dir.into(),
        }
    }

    #[must_use]
    pub fn package_dir(&self, scope: PackageScope) -> PathBuf {
        match scope {
            PackageScope::User => self.agent_dir.join("extensions"),
            PackageScope::Project => self.cwd.join(CONFIG_DIR_NAME).join("extensions"),
        }
    }

    fn source_for(&self, value: &str, scope: PackageScope) -> Result<ExtensionSource, String> {
        let source = ExtensionSource::parse(value)?;
        Ok(match source {
            // Pi resolves user-local paths from the agent directory and project
            // paths from the project, not from its managed install store.
            ExtensionSource::Path(path) if path.is_relative() => {
                ExtensionSource::Path(match scope {
                    PackageScope::User => self.agent_dir.join(path),
                    PackageScope::Project => self.cwd.join(path),
                })
            }
            source => source,
        })
    }

    /// Installs a source using Cargo's conventional cdylib artifact name.
    /// This keeps the CLI source-only: the supplied value never names or loads
    /// a prebuilt artifact.
    pub fn install_source(
        &self,
        source: &str,
        scope: PackageScope,
    ) -> Result<NativeExtensionInstall, String> {
        let source = self.source_for(source, scope)?;
        let name = match &source {
            ExtensionSource::Path(path) => cargo_library_name(path)?,
            ExtensionSource::Crate { name, .. } => name.clone(),
            ExtensionSource::Github { repo, package, .. } => package
                .as_deref()
                .map(Path::new)
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or(repo)
                .to_owned(),
        }
        .replace('-', "_");
        self.install_and_persist(
            &NativePackageSpec {
                source: source.canonical(),
                artifact: PathBuf::from(format!(
                    "{}{}{}",
                    std::env::consts::DLL_PREFIX,
                    name,
                    std::env::consts::DLL_SUFFIX
                )),
            },
            scope,
        )
    }

    /// Rebuilds one configured source, or every configured source in `scope`.
    pub fn update(&self, source: Option<&str>, scope: PackageScope) -> Result<usize, String> {
        let installs =
            NativeExtensionInstall::load_persisted(&self.package_dir(scope).join("source"))?;
        let requested = source
            .map(|value| {
                self.source_for(value, scope)
                    .map(|source| source.canonical())
            })
            .transpose()?;
        let selected: Vec<_> = installs
            .into_iter()
            .filter(|install| {
                requested
                    .as_deref()
                    .is_none_or(|value| install.receipt.source == value)
            })
            .collect();
        if source.is_some() && selected.is_empty() {
            return Err(format!("Package not found: {}", source.unwrap()));
        }
        for install in &selected {
            let artifact = install
                .artifact
                .file_name()
                .ok_or("installed artifact has no file name")?
                .into();
            self.install_and_persist(
                &NativePackageSpec {
                    source: install.receipt.source.clone(),
                    artifact,
                },
                scope,
            )?;
        }
        Ok(selected.len())
    }

    /// Installs only source-built native extensions and atomically records their
    /// provenance. Prebuilt artifacts are never used by this operation.
    pub fn install(
        &self,
        spec: &NativePackageSpec,
        scope: PackageScope,
    ) -> Result<NativeExtensionInstall, String> {
        let root = self.package_dir(scope);
        let source = self.source_for(&spec.source, scope)?;
        let artifact = Path::new("target").join("release").join(&spec.artifact);
        install_source(
            &source,
            &root.join("source"),
            &root.join("staging"),
            &artifact,
            &root.join("store"),
            self.installed_for(&source, &root)
                .map(|install| install.receipt.artifact_sha256),
        )
    }

    /// Pi-compatible name for installation. Native source installs are always
    /// persisted because their receipt is the loader's trust authority.
    pub fn install_and_persist(
        &self,
        spec: &NativePackageSpec,
        scope: PackageScope,
    ) -> Result<NativeExtensionInstall, String> {
        self.install(spec, scope)
    }

    #[must_use]
    pub fn list_configured_packages(&self) -> Vec<ConfiguredPackage> {
        [PackageScope::User, PackageScope::Project]
            .into_iter()
            .flat_map(|scope| {
                let root = self.package_dir(scope);
                NativeExtensionInstall::load_persisted(&root.join("source"))
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |install| ConfiguredPackage {
                        source: install.receipt.source.clone(),
                        scope,
                        filtered: false,
                        installed_path: install.resolve().ok().map(|(path, _)| path),
                    })
            })
            .collect()
    }

    #[must_use]
    pub fn get_installed_path(&self, source: &str, scope: PackageScope) -> Option<PathBuf> {
        self.list_configured_packages()
            .into_iter()
            .find(|package| package.scope == scope && package.source == source)
            .and_then(|package| package.installed_path)
    }

    /// Removes the persisted receipt and its application-managed authorization.
    /// The content-addressed store is deliberately retained for other receipts.
    pub fn remove(&self, source: &str, scope: PackageScope) -> Result<bool, String> {
        let root = self.package_dir(scope);
        let source = self.source_for(source, scope)?.canonical();
        let installs = root.join("source").join("native-extension-installs");
        let entries = match fs::read_dir(&installs) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            let Ok(bytes) = fs::read(&path) else { continue };
            let Ok(install) = serde_json::from_slice::<NativeExtensionInstall>(&bytes) else {
                continue;
            };
            if install.receipt.source == source {
                fs::remove_file(&path).map_err(|error| error.to_string())?;
                let trust = root
                    .join("native-extension-trust")
                    .join(format!("{}.json", install.receipt.source_sha256));
                let _ = fs::remove_file(trust);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn installed_for(
        &self,
        source: &ExtensionSource,
        root: &Path,
    ) -> Option<NativeExtensionInstall> {
        NativeExtensionInstall::load_persisted(&root.join("source"))
            .ok()?
            .into_iter()
            .find(|install| install.receipt.source == source.canonical())
    }
}

fn cargo_library_name(source: &Path) -> Result<String, String> {
    let manifest = fs::read_to_string(source.join("Cargo.toml"))
        .map_err(|error| format!("{}: {error}", source.join("Cargo.toml").display()))?;
    let mut section = "";
    let mut package_name = None;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }
        let name = value.trim().trim_matches('"');
        if section == "lib" {
            return Ok(name.to_owned());
        }
        if section == "package" {
            package_name = Some(name.to_owned());
        }
    }
    package_name.ok_or_else(|| "extension Cargo.toml has no package or lib name".into())
}
