//! Declarative flow-package metadata. Reading a manifest does not discover files,
//! resolve dependencies, compile Rust, or execute a build script.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use zf_core::identity::PackageId;

pub const PACKAGE_FORMAT_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "flow.json";
pub const ENTRY_FILE: &str = "flow.rs";

/// Bounds checked by pure validation; acquisition must also bound reads before
/// allocating these buffers. These are package limits, not Cargo build limits.
pub const MAX_PACKAGE_FILES: usize = 4096;
pub const MAX_SNAPSHOT_PACKAGES: usize = 1024;
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub const MAX_PACKAGE_FILE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;

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
            source.len() <= MAX_MANIFEST_BYTES,
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
        ensure!(
            self.files.len() <= MAX_PACKAGE_FILES,
            "too many package files"
        );
        ensure!(
            self.dependencies.len() <= MAX_SNAPSHOT_PACKAGES,
            "too many package dependencies"
        );
        for path in &self.files {
            validate_file_path(path)?;
            ensure!(
                paths.insert(path.as_str()),
                "duplicate package file: {path}"
            );
        }
        for path in &self.files {
            for (index, _) in path.match_indices('/') {
                ensure!(
                    !paths.contains(&path[..index]),
                    "package file is also a directory: {}",
                    &path[..index]
                );
            }
        }
        ensure!(
            paths.contains(self.entry.as_str()),
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

/// Exact portable package contents and its explicitly resolved local aliases.
/// Map keys are relative inventory paths and dependency aliases respectively.
/// Deserialization is structural only: call [`PackageSnapshot::validate`] before
/// trusting a received archive. No method reads files or resolves dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageNode {
    pub manifest_source: String,
    /// UTF-8 files serialize as exact strings; other bytes use `{ "base64": "..." }`.
    #[serde(with = "file_contents")]
    pub files: BTreeMap<String, Vec<u8>>,
    pub dependencies: BTreeMap<String, String>,
}

/// Decode each inventory value with bounds before copying text or allocating a
/// decoded binary buffer. The host must still bound the incoming JSON itself:
/// serde's parser may buffer a string before passing it to a visitor.
mod file_contents {
    use super::{MAX_PACKAGE_FILE_BYTES, MAX_PACKAGE_FILES, MAX_SNAPSHOT_BYTES};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};
    use serde::ser::{SerializeMap, SerializeStruct};
    use serde::{Deserializer, Serialize, Serializer};
    use std::{collections::BTreeMap, fmt};

    pub fn serialize<S: Serializer>(
        files: &BTreeMap<String, Vec<u8>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(files.len()))?;
        for (path, bytes) in files {
            map.serialize_entry(path, &FileBytes(bytes))?;
        }
        map.end()
    }

    struct FileBytes<'a>(&'a [u8]);

    impl Serialize for FileBytes<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            if let Ok(text) = std::str::from_utf8(self.0) {
                serializer.serialize_str(text)
            } else {
                let mut object = serializer.serialize_struct("BinaryFile", 1)?;
                object.serialize_field("base64", &STANDARD.encode(self.0))?;
                object.end()
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<String, Vec<u8>>, D::Error> {
        deserializer.deserialize_map(InventoryVisitor)
    }

    struct InventoryVisitor;

    impl<'de> Visitor<'de> for InventoryVisitor {
        type Value = BTreeMap<String, Vec<u8>>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an inventory of UTF-8 strings or strict base64 objects")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut files = BTreeMap::new();
            let mut remaining = MAX_SNAPSHOT_BYTES;
            while let Some(path) = map.next_key::<String>()? {
                if files.len() == MAX_PACKAGE_FILES {
                    return Err(A::Error::custom("too many package files"));
                }
                if files.contains_key(&path) {
                    return Err(A::Error::custom("duplicate package inventory path"));
                }
                let bytes = map.next_value_seed(FileVisitor { remaining })?;
                remaining -= bytes.len();
                files.insert(path, bytes);
            }
            Ok(files)
        }
    }

    struct FileVisitor {
        remaining: usize,
    }

    impl<'de> DeserializeSeed<'de> for FileVisitor {
        type Value = Vec<u8>;

        fn deserialize<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(self)
        }
    }

    impl<'de> Visitor<'de> for FileVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a UTF-8 string or an object containing only base64")
        }

        fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
            check_size(value.len(), self.remaining)?;
            Ok(value.as_bytes().to_vec())
        }

        fn visit_string<E: Error>(self, value: String) -> Result<Self::Value, E> {
            check_size(value.len(), self.remaining)?;
            Ok(value.into_bytes())
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            if map.next_key::<String>()?.as_deref() != Some("base64") {
                return Err(A::Error::custom("binary package file requires only base64"));
            }
            let bytes = map.next_value_seed(BinaryVisitor {
                remaining: self.remaining,
            })?;
            if map.next_key::<String>()?.is_some() {
                return Err(A::Error::custom("binary package file requires only base64"));
            }
            Ok(bytes)
        }
    }

    struct BinaryVisitor {
        remaining: usize,
    }

    impl<'de> DeserializeSeed<'de> for BinaryVisitor {
        type Value = Vec<u8>;

        fn deserialize<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_str(self)
        }
    }

    impl Visitor<'_> for BinaryVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("canonical padded base64 for a non-UTF-8 file")
        }

        fn visit_str<E: Error>(self, encoded: &str) -> Result<Self::Value, E> {
            if encoded.len() > MAX_PACKAGE_FILE_BYTES.div_ceil(3) * 4 {
                return Err(E::custom("package file exceeds 16 MiB"));
            }
            if !encoded.len().is_multiple_of(4) {
                return Err(E::custom("invalid padded base64 package file"));
            }
            let padding = if encoded.ends_with("==") {
                2
            } else {
                usize::from(encoded.ends_with('='))
            };
            let decoded_len = (encoded.len() / 4 * 3).saturating_sub(padding);
            check_size(decoded_len, self.remaining)?;
            let bytes = STANDARD.decode(encoded).map_err(E::custom)?;
            if std::str::from_utf8(&bytes).is_ok() {
                return Err(E::custom("UTF-8 package files must use strings"));
            }
            Ok(bytes)
        }
    }

    fn check_size<E: Error>(length: usize, remaining: usize) -> Result<(), E> {
        if length > MAX_PACKAGE_FILE_BYTES {
            return Err(E::custom("package file exceeds 16 MiB"));
        }
        if length > remaining {
            return Err(E::custom("package inventory exceeds 128 MiB"));
        }
        Ok(())
    }
}

impl PackageNode {
    /// Parse the preserved manifest without normalizing its source bytes.
    pub fn manifest(&self) -> Result<FlowPackageManifest> {
        FlowPackageManifest::parse(&self.manifest_source)
    }

    /// The declared flow identity, independent of any Cargo crate name.
    pub fn package_id(&self) -> Result<PackageId> {
        Ok(self.manifest()?.id)
    }

    /// Borrow the exact UTF-8 entry source. Other inventory files may be binary.
    pub fn entry_source(&self) -> Result<&str> {
        let source = self
            .files
            .get(ENTRY_FILE)
            .context("missing package entry")?;
        std::str::from_utf8(source).context("package entry is not UTF-8")
    }

    /// SHA-256 of exact manifest bytes and the sorted path/byte inventory.
    /// Hashing does not validate the node or perform host-dependent normalization.
    pub fn content_revision(&self) -> String {
        let mut hash = revision_hasher(b"zedflow.package.content.v1");
        hash_part(&mut hash, self.manifest_source.as_bytes());
        for (path, bytes) in &self.files {
            hash_part(&mut hash, path.as_bytes());
            hash_part(&mut hash, bytes);
        }
        format!("{:x}", hash.finalize())
    }

    /// SHA-256 of sorted aliases and their resolved node revisions. Child node
    /// revisions recursively pin the complete dependency closure.
    pub fn dependencies_revision(&self) -> String {
        let mut hash = revision_hasher(b"zedflow.package.dependencies.v1");
        for (alias, revision) in &self.dependencies {
            hash_part(&mut hash, alias.as_bytes());
            hash_part(&mut hash, revision.as_bytes());
        }
        format!("{:x}", hash.finalize())
    }

    /// Combined package revision, distinct from either component revision.
    pub fn revision(&self) -> String {
        let mut hash = revision_hasher(b"zedflow.package.revision.v1");
        hash_part(&mut hash, self.content_revision().as_bytes());
        hash_part(&mut hash, self.dependencies_revision().as_bytes());
        format!("{:x}", hash.finalize())
    }

    fn validate_inventory(&self) -> Result<FlowPackageManifest> {
        ensure!(
            self.files.len() <= MAX_PACKAGE_FILES,
            "too many package files"
        );
        let manifest = self.manifest()?;
        let declared: BTreeSet<&str> = manifest.files.iter().map(String::as_str).collect();
        let captured: BTreeSet<&str> = self.files.keys().map(String::as_str).collect();
        ensure!(
            declared == captured,
            "package inventory differs from manifest"
        );
        ensure!(
            manifest.dependencies.keys().eq(self.dependencies.keys()),
            "resolved dependency aliases differ from manifest"
        );
        for (path, bytes) in &self.files {
            ensure!(
                bytes.len() <= MAX_PACKAGE_FILE_BYTES,
                "package file exceeds 16 MiB: {path}"
            );
        }
        self.entry_source()?;
        self.captured_bytes()?;
        Ok(manifest)
    }

    fn captured_bytes(&self) -> Result<usize> {
        let mut total = self.manifest_source.len();
        for bytes in self.files.values() {
            total = total
                .checked_add(bytes.len())
                .context("snapshot size overflow")?;
            ensure!(total <= MAX_SNAPSHOT_BYTES, "snapshot exceeds 128 MiB");
        }
        Ok(total)
    }
}

/// A complete, deduplicated dependency closure. `root` and every map key are
/// lowercase SHA-256 package revisions, never filesystem paths or Cargo names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackageSnapshot {
    pub root: String,
    pub packages: BTreeMap<String, PackageNode>,
}

impl PackageSnapshot {
    /// Freeze already acquired bytes and explicitly supplied dependency snapshots.
    /// Shared dependencies are stored once by revision.
    ///
    /// # Errors
    /// Rejects invalid dependencies, inventories, aliases, conflicting identities
    /// or content, and closures exceeding the documented limits.
    pub fn capture(
        manifest_source: String,
        files: BTreeMap<String, Vec<u8>>,
        dependencies: BTreeMap<String, Self>,
    ) -> Result<Self> {
        let manifest = FlowPackageManifest::parse(&manifest_source)?;
        ensure!(
            manifest.dependencies.keys().eq(dependencies.keys()),
            "resolved dependency aliases differ from manifest"
        );
        let mut packages = BTreeMap::new();
        let mut resolutions = BTreeMap::new();
        let mut total_bytes = 0usize;
        for (alias, snapshot) in dependencies {
            snapshot
                .validate()
                .with_context(|| format!("invalid dependency: {alias}"))?;
            resolutions.insert(alias, snapshot.root);
            for (revision, node) in snapshot.packages {
                match packages.entry(revision) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        add_captured_bytes(&mut total_bytes, &node)?;
                        entry.insert(node);
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => {
                        ensure!(
                            entry.get() == &node,
                            "conflicting package revision contents"
                        );
                    }
                }
            }
            ensure!(
                packages.len() <= MAX_SNAPSHOT_PACKAGES,
                "too many snapshot packages"
            );
        }
        let node = PackageNode {
            manifest_source,
            files,
            dependencies: resolutions,
        };
        node.validate_inventory()?;
        add_captured_bytes(&mut total_bytes, &node)?;
        let root = node.revision();
        match packages.entry(root.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(node);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                ensure!(
                    entry.get() == &node,
                    "conflicting root package revision contents"
                );
            }
        }
        let snapshot = Self { root, packages };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Borrow the root node. This does not establish archive integrity; validate
    /// snapshots received through serde before using their contents.
    pub fn root_node(&self) -> Result<&PackageNode> {
        self.packages
            .get(&self.root)
            .context("missing snapshot root package")
    }

    /// Parse the exact captured root manifest.
    pub fn root_manifest(&self) -> Result<FlowPackageManifest> {
        self.root_node()?.manifest()
    }

    /// Verify inventory, resolved aliases, hash integrity, and the exact reachable
    /// acyclic closure. Distinct revisions for one flow identity are rejected;
    /// unrelated flow identities may use identical human or Cargo names.
    ///
    /// # Errors
    /// Rejects malformed hashes, missing or orphaned nodes, cycles, corruption,
    /// conflicting flow identities, invalid manifests, and resource limit excess.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.packages.len() <= MAX_SNAPSHOT_PACKAGES,
            "too many snapshot packages"
        );
        validate_revision(&self.root)?;
        self.root_node()?;
        let mut total_bytes = 0usize;
        let mut identities = BTreeMap::new();
        for (revision, node) in &self.packages {
            validate_revision(revision)?;
            let manifest = node
                .validate_inventory()
                .with_context(|| format!("invalid package node: {revision}"))?;
            if let Some(previous) = identities.insert(manifest.id.clone(), revision) {
                ensure!(
                    previous == revision,
                    "conflicting revisions for flow identity: {}",
                    manifest.id
                );
            }
            add_captured_bytes(&mut total_bytes, node)?;
            for dependency in node.dependencies.values() {
                validate_revision(dependency)?;
                ensure!(
                    self.packages.contains_key(dependency),
                    "missing dependency package: {dependency}"
                );
            }
        }
        // Iterative depth-first traversal also bounds the call stack for hostile
        // deserialized graphs. A node is visited once even for a diamond closure.
        let mut active = BTreeSet::new();
        let mut complete = BTreeSet::new();
        let mut pending = vec![(self.root.as_str(), false)];
        while let Some((revision, exiting)) = pending.pop() {
            if exiting {
                active.remove(revision);
                complete.insert(revision);
                continue;
            }
            if complete.contains(revision) {
                continue;
            }
            ensure!(active.insert(revision), "cyclic package dependency closure");
            pending.push((revision, true));
            let node = self
                .packages
                .get(revision)
                .context("missing dependency package")?;
            for dependency in node.dependencies.values() {
                pending.push((dependency.as_str(), false));
            }
        }
        ensure!(
            complete.len() == self.packages.len(),
            "snapshot contains orphan package nodes"
        );
        for (revision, node) in &self.packages {
            ensure!(
                &node.revision() == revision,
                "package revision integrity mismatch: {revision}"
            );
        }
        Ok(())
    }
}

fn add_captured_bytes(total: &mut usize, node: &PackageNode) -> Result<()> {
    *total = total
        .checked_add(node.captured_bytes()?)
        .context("snapshot size overflow")?;
    ensure!(*total <= MAX_SNAPSHOT_BYTES, "snapshot exceeds 128 MiB");
    Ok(())
}

fn revision_hasher(domain: &[u8]) -> Sha256 {
    let mut hash = Sha256::new();
    hash_part(&mut hash, domain);
    hash
}

fn hash_part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}

fn validate_revision(revision: &str) -> Result<()> {
    ensure!(
        revision.len() == 64
            && revision
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid package revision hash: {revision}"
    );
    Ok(())
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
