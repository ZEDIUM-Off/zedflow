#![allow(unsafe_code)]

use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use libloading::Library;
use sha2::{Digest, Sha256};

use super::super::source_info::{SourceOrigin, SourceScope, create_synthetic_source_info};
use super::abi::{
    AbiBytes, AbiEntryV1, AbiHandle, AbiOwnedBytes, AbiTableHeader, AbiV1, JsonEnvelope,
    validate_handle, validate_table, validate_table_header,
};
use super::types::{Extension, ExtensionFactory, ExtensionRuntime, LoadExtensionsResult};

static CACHE: OnceLock<Mutex<HashMap<String, Extension>>> = OnceLock::new();
static LIBRARIES: OnceLock<Mutex<Vec<Library>>> = OnceLock::new();
static SNAPSHOT_COUNTER: AtomicU64 = AtomicU64::new(0);
fn cache() -> &'static Mutex<HashMap<String, Extension>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
fn libraries() -> &'static Mutex<Vec<Library>> {
    LIBRARIES.get_or_init(|| Mutex::new(Vec::new()))
}

/// A native artifact is executable only after its exact digest and explicit
/// source trust have been checked. Native extensions are trusted code, not a sandbox.
#[derive(Debug, Clone)]
pub struct NativeExtensionArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub trusted: bool,
}

pub struct NativeExtension {
    table: AbiV1,
    handle: Option<AbiHandle>,
}

impl NativeExtension {
    /// Loads and initializes a verified `cdylib`. The library is deliberately
    /// retained globally for process lifetime; v1 never unloads native code.
    pub fn load(
        artifact: &NativeExtensionArtifact,
        request: &JsonEnvelope,
    ) -> Result<Self, String> {
        if !artifact.trusted {
            return Err("native extension artifact is not trusted".into());
        }
        let snapshot = verified_artifact_snapshot(artifact)?;
        let library = unsafe { Library::new(&snapshot.path) }.map_err(|error| {
            format!(
                "failed to load native extension {}: {error}",
                artifact.path.display()
            )
        })?;
        let entry: libloading::Symbol<AbiEntryV1> =
            unsafe { library.get(b"zedflow_extension_abi_v1\0") }
                .map_err(|error| format!("missing zedflow_extension_abi_v1: {error}"))?;
        let table_pointer = unsafe { entry() };
        if table_pointer.is_null()
            || !(table_pointer as usize).is_multiple_of(std::mem::align_of::<AbiV1>())
        {
            return Err("native extension returned an invalid ABI table pointer".into());
        }
        let header = unsafe { *(table_pointer.cast::<AbiTableHeader>()) };
        validate_table_header(header)?;
        let table = unsafe { *table_pointer };
        validate_table(&table)?;
        let request = request.encode()?;
        let input = AbiBytes {
            ptr: request.as_ptr(),
            len: request.len() as u64,
        };
        let mut handle = AbiHandle {
            kind: 0,
            reserved: 0,
            raw: 0,
            generation: 0,
        };
        let status = (table.create.expect("validated ABI table"))(&input, &mut handle);
        if status != 0 || validate_handle(handle, handle.generation).is_err() {
            return Err(format!(
                "native extension initialization failed with status {status}"
            ));
        }
        libraries()
            .lock()
            .map_err(|_| "native library registry poisoned")?
            .push(library);
        Ok(Self {
            table,
            handle: Some(handle),
        })
    }

    pub fn call(&self, request: &JsonEnvelope) -> Result<JsonEnvelope, String> {
        let handle = self.handle.ok_or("native extension is shut down")?;
        validate_handle(handle, handle.generation)?;
        let request = request.encode()?;
        let input = AbiBytes {
            ptr: request.as_ptr(),
            len: request.len() as u64,
        };
        let mut output = AbiOwnedBytes {
            ptr: std::ptr::null_mut(),
            len: 0,
        };
        let status = (self.table.call.expect("validated ABI table"))(handle, &input, &mut output);
        if status != 0 {
            return Err(format!("native extension call failed with status {status}"));
        }
        let result = (|| {
            let bytes = unsafe { checked_owned_bytes(output)? };
            JsonEnvelope::parse(&bytes)
        })();
        (self.table.free_bytes.expect("validated ABI table"))(output);
        result
    }

    /// Idempotently disables this instance. Its library remains loaded.
    pub fn shutdown(&mut self) -> Result<(), String> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        let status = (self.table.destroy.expect("validated ABI table"))(handle);
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "native extension shutdown failed with status {status}"
            ))
        }
    }
}

impl Drop for NativeExtension {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct VerifiedArtifactSnapshot {
    path: PathBuf,
    directory: PathBuf,
}

impl Drop for VerifiedArtifactSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn verified_artifact_snapshot(
    artifact: &NativeExtensionArtifact,
) -> Result<VerifiedArtifactSnapshot, String> {
    let mut source = fs::File::open(&artifact.path)
        .map_err(|error| format!("cannot open native extension artifact: {error}"))?;
    if !source
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("native extension artifact is not a file".into());
    }

    let directory = std::env::temp_dir().join(format!(
        "zedflow-native-extension-{}-{}",
        std::process::id(),
        SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).map_err(|error| error.to_string())?;
    let path = directory.join("artifact");
    let result = (|| {
        let mut snapshot = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| error.to_string())?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            snapshot
                .write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
        }
        snapshot.sync_all().map_err(|error| error.to_string())?;
        let actual = format!("{:x}", digest.finalize());
        if !actual.eq_ignore_ascii_case(&artifact.sha256) {
            return Err("native extension artifact SHA-256 mismatch".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&directory);
        return Err(error);
    }
    Ok(VerifiedArtifactSnapshot { path, directory })
}

/// # Safety
/// `output` comes from a trusted, SDK-built extension. Pointer validity is its
/// ABI precondition; this only enforces null/length and message-size contracts.
unsafe fn checked_owned_bytes(output: AbiOwnedBytes) -> Result<Vec<u8>, String> {
    if output.len > super::abi::MAX_JSON_BYTES as u64 {
        return Err(format!(
            "extension JSON exceeds {} bytes",
            super::abi::MAX_JSON_BYTES
        ));
    }
    if output.len == 0 {
        return Ok(Vec::new());
    }
    if output.ptr.is_null() {
        return Err("native extension returned null bytes with nonzero length".into());
    }
    Ok(unsafe { std::slice::from_raw_parts(output.ptr, output.len as usize) }.to_vec())
}

pub fn clear_extension_cache() {
    if let Ok(mut cache) = cache().lock() {
        cache.clear();
    }
}

#[must_use]
pub fn create_extension_runtime() -> ExtensionRuntime {
    ExtensionRuntime::default()
}

pub fn load_extension_from_factory(
    name: impl Into<String>,
    factory: ExtensionFactory,
    runtime: &mut ExtensionRuntime,
) -> Result<Extension, String> {
    let name = name.into();
    factory(runtime).map_err(|error| error.message)?;
    let source = create_synthetic_source_info(
        name.clone(),
        "inline",
        Some(SourceScope::Temporary),
        Some(SourceOrigin::TopLevel),
        None,
    );
    let extension = Extension {
        name: name.clone(),
        source,
    };
    if let Ok(mut values) = cache().lock() {
        values.insert(name, extension.clone());
    }
    Ok(extension)
}

#[must_use]
pub fn load_extensions(paths: &[impl AsRef<Path>]) -> LoadExtensionsResult {
    let mut result = LoadExtensionsResult::default();
    for path in paths {
        let path = path.as_ref();
        if !path.exists() {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("extension")
            .to_owned();
        let source = create_synthetic_source_info(
            path.display().to_string(),
            "local",
            Some(SourceScope::Project),
            Some(SourceOrigin::TopLevel),
            path.parent().map(|v| v.display().to_string()),
        );
        result.extensions.push(Extension { name, source });
    }
    result
}

#[must_use]
pub fn load_extensions_cached(paths: &[impl AsRef<Path>]) -> LoadExtensionsResult {
    let mut result = LoadExtensionsResult::default();
    for path in paths {
        let key = path.as_ref().display().to_string();
        if let Ok(values) = cache().lock() {
            if let Some(extension) = values.get(&key) {
                result.extensions.push(extension.clone());
                continue;
            }
        }
        let loaded = load_extensions(std::slice::from_ref(path));
        for extension in loaded.extensions {
            if let Ok(mut values) = cache().lock() {
                values.insert(key.clone(), extension.clone());
            }
            result.extensions.push(extension);
        }
        result.errors.extend(loaded.errors);
    }
    result
}

#[must_use]
pub fn discover_and_load_extensions(cwd: impl AsRef<Path>) -> LoadExtensionsResult {
    let dir = cwd.as_ref().join(".pi/extensions");
    let Ok(entries) = fs::read_dir(dir) else {
        return LoadExtensionsResult::default();
    };
    let paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    load_extensions(&paths)
}
