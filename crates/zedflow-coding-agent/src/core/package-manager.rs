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
    pub installed_path: Option<PathBuf>,
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
            ExtensionSource::Path(path) if path.is_relative() => {
                ExtensionSource::Path(self.package_dir(scope).join(path))
            }
            source => source,
        })
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
                        installed_path: install.resolve().ok().map(|(path, _)| path),
                    })
            })
            .collect()
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
