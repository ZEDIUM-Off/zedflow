#![allow(unsafe_code)]

use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use libloading::Library;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::super::source_info::{SourceOrigin, SourceScope, create_synthetic_source_info};
use super::abi::{
    AbiBytes, AbiEntryV1, AbiHandle, AbiOwnedBytes, AbiTableHeader, AbiV1, JsonEnvelope,
    validate_handle, validate_table, validate_table_header,
};
use super::{
    runner::ExtensionRunner,
    types::{
        Extension, ExtensionEventKind, ExtensionFactory, ExtensionHandler, ExtensionRuntime,
        LoadExtensionsResult, RegisteredCommand, define_tool,
    },
};

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
    inner: Arc<NativeExtensionInner>,
}

struct NativeExtensionInner {
    table: AbiV1,
    handle: Mutex<Option<AbiHandle>>,
    call_lock: Mutex<()>,
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
        let table_pointer = catch_unwind(AssertUnwindSafe(|| unsafe { entry() }))
            .map_err(|_| "native extension ABI entry panicked")?;
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
        let status =
            ffi_status(|| (table.create.expect("validated ABI table"))(&input, &mut handle))?;
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
            inner: Arc::new(NativeExtensionInner {
                table,
                handle: Mutex::new(Some(handle)),
                call_lock: Mutex::new(()),
            }),
        })
    }

    pub fn call(&self, request: &JsonEnvelope) -> Result<JsonEnvelope, String> {
        self.inner.call(request)
    }

    /// Registers ABI-declared runtime entries and returns lifecycle handlers.
    /// The activation request is an ordinary ABI call so v1 needs no additional entry point.
    pub fn activate(
        &self,
        runtime: &mut ExtensionRuntime,
    ) -> Result<Vec<(ExtensionEventKind, ExtensionHandler)>, String> {
        let reply = self.call(&JsonEnvelope {
            version: super::abi::ABI_V1,
            payload: json!({"kind": "activate"}),
        })?;
        let api = reply
            .payload
            .get("api")
            .and_then(Value::as_object)
            .ok_or("native extension activation did not return an API snapshot")?;
        // Validate every registration before changing the runtime.
        let tools = api_names(api, "tools", "tool")?;
        let commands = api_names(api, "commands", "command")?;
        let events = api_names(api, "events", "event")?
            .into_iter()
            .map(|name| native_event_kind(&name).map(|kind| (name, kind)))
            .collect::<Result<Vec<_>, _>>()?;
        let providers = api
            .get("providers")
            .and_then(Value::as_object)
            .ok_or("native extension activation did not return providers")?
            .iter()
            .map(|(name, config)| (name.clone(), config.clone()))
            .collect::<Vec<_>>();
        let extension = Arc::clone(&self.inner);

        for name in tools {
            let native = Arc::clone(&extension);
            let tool_name = name.clone();
            runtime.register_tool(
                define_tool(name, "native extension tool"),
                Arc::new(move |arguments, context| {
                    native
                        .call(&JsonEnvelope {
                            version: super::abi::ABI_V1,
                            payload: json!({
                                "kind": "tool",
                                "name": tool_name,
                                "arguments": arguments,
                                "context": native_context(context),
                            }),
                        })
                        .map_err(native_error)
                        .map(|reply| reply.payload.get("result").cloned().unwrap_or(Value::Null))
                }),
            );
        }
        for name in commands {
            let native = Arc::clone(&extension);
            let command_name = name.clone();
            runtime.register_command(
                RegisteredCommand {
                    name,
                    description: "native extension command".into(),
                },
                Arc::new(move |args, context| {
                    native
                        .call(&JsonEnvelope {
                            version: super::abi::ABI_V1,
                            payload: json!({
                                "kind": "command",
                                "name": command_name,
                                "args": args,
                                "context": native_context(context),
                            }),
                        })
                        .map_err(native_error)
                        .map(|reply| super::types::SessionActionResult {
                            cancelled: reply
                                .payload
                                .get("result")
                                .and_then(|value| value.get("cancelled"))
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        })
                }),
            );
        }
        for (name, config) in providers {
            runtime.register_provider(super::types::ProviderConfig { name, config });
        }
        Ok(events
            .into_iter()
            .map(|(event_name, kind)| {
                let native = Arc::clone(&extension);
                (
                    kind,
                    Arc::new(
                        move |event: &super::types::ExtensionEvent,
                              context: &mut super::types::ExtensionContext| {
                        native
                            .call(&JsonEnvelope {
                                version: super::abi::ABI_V1,
                                payload: json!({
                                    "kind": "event",
                                    "event": event_name,
                                    "data": event.data,
                                    "context": native_context(context),
                                }),
                            })
                            .map_err(native_error)
                            .map(|reply| reply.payload.get("result").cloned())
                        },
                    ) as ExtensionHandler,
                )
            })
            .collect())
    }

    /// Idempotently disables this instance. Its library remains loaded.
    pub fn shutdown(&self) -> Result<(), String> {
        self.inner.shutdown()
    }
}

impl Drop for NativeExtension {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

impl NativeExtensionInner {
    fn call(&self, request: &JsonEnvelope) -> Result<JsonEnvelope, String> {
        // ABI v1 instances are mutable and may not be called concurrently.
        let _call_lock = self
            .call_lock
            .lock()
            .map_err(|_| "native extension call lock poisoned")?;
        let handle = *self
            .handle
            .lock()
            .map_err(|_| "native extension handle lock poisoned")?
            .as_ref()
            .ok_or("native extension is shut down")?;
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
        let status = ffi_status(|| {
            (self.table.call.expect("validated ABI table"))(handle, &input, &mut output)
        })?;
        if status != 0 {
            return Err(format!("native extension call failed with status {status}"));
        }
        let result = (|| {
            let bytes = unsafe { checked_owned_bytes(output)? };
            JsonEnvelope::parse(&bytes)
        })();
        ffi_void(|| (self.table.free_bytes.expect("validated ABI table"))(output))?;
        result
    }

    fn shutdown(&self) -> Result<(), String> {
        let _call_lock = self
            .call_lock
            .lock()
            .map_err(|_| "native extension call lock poisoned")?;
        let Some(handle) = self
            .handle
            .lock()
            .map_err(|_| "native extension handle lock poisoned")?
            .take()
        else {
            return Ok(());
        };
        let status = ffi_status(|| (self.table.destroy.expect("validated ABI table"))(handle))?;
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "native extension shutdown failed with status {status}"
            ))
        }
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

fn api_names(
    api: &serde_json::Map<String, Value>,
    field: &str,
    kind: &str,
) -> Result<Vec<String>, String> {
    api.get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("native extension activation did not return {field}"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| format!("native extension returned invalid {kind} registration"))
        })
        .collect()
}

fn native_event_kind(name: &str) -> Result<ExtensionEventKind, String> {
    use ExtensionEventKind::*;
    let kind = match name {
        "project_trust" => ProjectTrust,
        "resources_discover" => ResourcesDiscover,
        "session_start" => SessionStart,
        "session_info_changed" => SessionInfoChanged,
        "session_before_switch" => SessionBeforeSwitch,
        "session_before_fork" => SessionBeforeFork,
        "session_before_compact" => SessionBeforeCompact,
        "session_compact" => SessionCompact,
        "session_shutdown" => SessionShutdown,
        "session_before_tree" => SessionBeforeTree,
        "session_tree" => SessionTree,
        "context" => Context,
        "before_provider_request" => BeforeProviderRequest,
        "before_provider_headers" => BeforeProviderHeaders,
        "after_provider_response" => AfterProviderResponse,
        "before_agent_start" => BeforeAgentStart,
        "agent_start" => AgentStart,
        "agent_end" => AgentEnd,
        "turn_start" => TurnStart,
        "turn_end" => TurnEnd,
        "message_start" => MessageStart,
        "message_update" => MessageUpdate,
        "message_end" => MessageEnd,
        "tool_execution_start" => ToolExecutionStart,
        "tool_execution_update" => ToolExecutionUpdate,
        "tool_execution_end" => ToolExecutionEnd,
        "model_select" => ModelSelect,
        "thinking_level_select" => ThinkingLevelSelect,
        "user_bash" => UserBash,
        "input" => Input,
        "tool_call" => ToolCall,
        "tool_result" => ToolResult,
        _ => {
            return Err(format!(
                "native extension returned unknown event registration: {name}"
            ));
        }
    };
    Ok(kind)
}

fn native_context(context: &super::types::ExtensionContext) -> Value {
    json!({
        "cwd": context.cwd,
        "hasUi": context.has_ui,
        "generation": context.generation,
    })
}

fn native_error(message: String) -> super::types::ExtensionError {
    super::types::ExtensionError {
        message,
        source: None,
    }
}

fn ffi_status(operation: impl FnOnce() -> i32) -> Result<i32, String> {
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| "native extension ABI call panicked".into())
}

fn ffi_void(operation: impl FnOnce()) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| "native extension ABI free_bytes panicked".into())
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

/// Loads trusted native artifacts and activates them into a runner-owned runtime.
pub fn load_native_extensions(
    artifacts: &[NativeExtensionArtifact],
    request: &JsonEnvelope,
) -> Result<ExtensionRunner, String> {
    let mut extensions = Vec::with_capacity(artifacts.len());
    let mut native_extensions = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        native_extensions.push(NativeExtension::load(artifact, request)?);
        let name = artifact
            .path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("native-extension")
            .to_owned();
        extensions.push(Extension {
            name,
            source: create_synthetic_source_info(
                artifact.path.display().to_string(),
                "native",
                Some(SourceScope::Project),
                Some(SourceOrigin::TopLevel),
                artifact
                    .path
                    .parent()
                    .map(|path| path.display().to_string()),
            ),
        });
    }
    ExtensionRunner::from_native_extensions(extensions, native_extensions)
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
        if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ts" | "js")
        ) {
            result.errors.push(native_error(format!(
                "deferred TypeScript/jiti extension {}: JavaScript extension runtimes are not available",
                path.display()
            )));
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

fn is_extension_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("ts" | "js" | "rs")
    )
}

fn resolve_extension_entries(dir: &Path) -> Vec<PathBuf> {
    let package_entries = fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|contents| serde_json::from_str::<Value>(&contents).ok())
        .and_then(|manifest| manifest["pi"]["extensions"].as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(|entry| dir.join(entry)))
        .filter(|path| path.is_file() && is_extension_file(path))
        .collect::<Vec<_>>();
    if !package_entries.is_empty() {
        return package_entries;
    }

    ["index.ts", "index.js"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .into_iter()
        .collect()
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
        .flat_map(|path| {
            if path.is_file() {
                is_extension_file(&path)
                    .then_some(path)
                    .into_iter()
                    .collect()
            } else if path.is_dir() {
                resolve_extension_entries(&path)
            } else {
                Vec::new()
            }
        })
        .collect();
    load_extensions(&paths)
}
