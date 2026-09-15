//! Declarative flow-package metadata. Reading a manifest does not discover files,
//! resolve dependencies, compile Rust, or execute a build script.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use zf_core::identity::PackageId;

pub const PACKAGE_FORMAT_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "flow.json";
pub const ENTRY_FILE: &str = "flow.rs";

/// A declared local package is resolved relative to the containing package.
/// Storage must freeze its complete dependency closure before compilation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalPackageDependency {
    pub path: String,
}

/// The package version is independent of the Rust flow format. `id` identifies
/// the flow; dependency aliases and generated Cargo crate names do not replace it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowPackageManifest {
    pub format_version: u32,
    pub id: PackageId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub entry: String,
    /// Paths relative to the package. The manifest itself is captured separately.
    pub files: Vec<String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, LocalPackageDependency>,
}

impl FlowPackageManifest {
    /// Parse declared metadata without accessing the filesystem or network.
    ///
    /// # Errors
    /// Rejects malformed metadata, unsupported package versions, ambiguous paths,
    /// missing entry files and reserved generated/build files. Symlink containment
    /// and dependency resolution must additionally be checked by storage.
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            source.len() <= 1024 * 1024,
            "package manifest exceeds 1 MiB"
        );
        let manifest: Self =
            serde_json::from_str(source).context("invalid flow package manifest")?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate a manifest constructed programmatically, using the same rules as
    /// `parse`. This establishes structure, not the existence of its resources.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.format_version == PACKAGE_FORMAT_VERSION,
            "unsupported flow package version"
        );
        ensure!(
            portable_component(self.id.as_str()),
            "invalid flow package identity"
        );
        ensure!(
            !self.name.trim().is_empty(),
            "flow package name is required"
        );
        ensure!(
            self.entry == ENTRY_FILE,
            "flow package entry must be flow.rs"
        );
        let mut paths = BTreeSet::new();
        for path in &self.files {
            validate_file_path(path)?;
            ensure!(paths.insert(path), "duplicate package file: {path}");
        }
        ensure!(
            paths.contains(&self.entry),
            "flow package entry is not declared in files"
        );
        for (alias, dependency) in &self.dependencies {
            ensure!(
                !alias.is_empty()
                    && alias
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-')),
                "invalid package dependency alias: {alias}"
            );
            // Parent components are permitted only for explicitly declared
            // dependencies. Their resolved snapshots belong to the package closure.
            ensure!(
                !dependency.path.is_empty()
                    && dependency
                        .path
                        .split('/')
                        .all(|part| part == ".." || portable_component(part)),
                "dependency path must be relative: {}",
                dependency.path
            );
        }
        Ok(())
    }
}

fn portable_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

/// Validate an inventory path without interpreting it using host-specific path
/// rules. The same declaration must remain relative on Linux, macOS and Windows.
pub fn validate_file_path(path: &str) -> Result<()> {
    ensure!(
        path.split('/').all(portable_component),
        "invalid package file path: {path}"
    );
    ensure!(
        path != MANIFEST_FILE,
        "flow.json is captured separately from package files"
    );
    for part in path.split('/') {
        ensure!(
            !matches!(
                part,
                "target" | "node_modules" | ".git" | "build.rs" | ".env"
            ) && !part.starts_with(".env."),
            "reserved package file path: {path}"
        );
    }
    Ok(())
}
