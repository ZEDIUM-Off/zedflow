//! Rust SDK for `zedflow` native extension `cdylib`s.

#![allow(unsafe_code)]

use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};

use serde_json::{Value, json};

use crate::extensions::{
    ABI_HANDLE_EXTENSION, ABI_OK, ABI_V1, AbiBytes, AbiHandle, AbiOwnedBytes, AbiStatus,
    JsonEnvelope,
};

pub use serde_json::Value as JsonValue;

/// Deterministic public SDK surface for the Codex cache-probe tool loop.
pub use crate::agent_session::{
    CodexCacheProbeAssistant, CodexCacheProbeMessage, CodexCacheProbeSession, CodexCacheProbeTurn,
    CodexCacheProbeUsage,
};

/// The small host-facing surface available to a native extension.
#[derive(Debug, Default)]
pub struct ExtensionApi {
    tools: Vec<String>,
    commands: Vec<String>,
    providers: BTreeMap<String, Value>,
    events: Vec<String>,
    ui: Vec<Value>,
}

impl ExtensionApi {
    pub fn register_tool(&mut self, name: impl Into<String>) {
        self.tools.push(name.into());
    }
    pub fn register_command(&mut self, name: impl Into<String>) {
        self.commands.push(name.into());
    }
    pub fn register_provider(&mut self, name: impl Into<String>, config: Value) {
        self.providers.insert(name.into(), config);
    }
    pub fn on_event(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }
    pub fn show_ui(&mut self, value: Value) {
        self.ui.push(value);
    }

    #[must_use]
    pub fn snapshot(&self) -> Value {
        json!({
            "tools": self.tools,
            "commands": self.commands,
            "providers": self.providers,
            "events": self.events,
            "ui": self.ui,
        })
    }
}

/// Implement this trait and use [`export_extension!`] to make a `cdylib`.
pub trait Extension: Default + Send + 'static {
    fn initialize(&mut self, _api: &mut ExtensionApi, _request: Value) -> Result<(), String> {
        Ok(())
    }
    fn invoke(&mut self, _api: &mut ExtensionApi, request: Value) -> Result<Value, String> {
        Ok(request)
    }
    fn shutdown(&mut self, _api: &mut ExtensionApi) -> Result<(), String> {
        Ok(())
    }
}

struct Instance<E> {
    extension: E,
    api: ExtensionApi,
}

/// # Safety
/// The ABI caller supplies valid pointers for the duration of this call.
pub unsafe fn create<E: Extension>(input: *const AbiBytes, output: *mut AbiHandle) -> AbiStatus {
    guarded(|| {
        if output.is_null() {
            return Err("null extension output".into());
        }
        let request = unsafe { input_json(input)? };
        let mut extension = E::default();
        let mut api = ExtensionApi::default();
        extension.initialize(&mut api, request.payload)?;
        unsafe {
            output.write(AbiHandle {
                kind: ABI_HANDLE_EXTENSION,
                reserved: 0,
                raw: Box::into_raw(Box::new(Instance { extension, api })) as u64,
                generation: 1,
            });
        }
        Ok(())
    })
}

/// # Safety
/// The ABI caller supplies a handle created by [`create`] and valid pointers.
pub unsafe fn call<E: Extension>(
    handle: AbiHandle,
    input: *const AbiBytes,
    output: *mut AbiOwnedBytes,
) -> AbiStatus {
    guarded(|| {
        if handle.kind != ABI_HANDLE_EXTENSION || handle.raw == 0 {
            return Err("invalid extension handle".into());
        }
        if output.is_null() {
            return Err("null extension output".into());
        }
        let request = unsafe { input_json(input)? };
        let instance = unsafe { &mut *(handle.raw as *mut Instance<E>) };
        let result = instance
            .extension
            .invoke(&mut instance.api, request.payload)?;
        let bytes = JsonEnvelope {
            version: ABI_V1,
            payload: json!({"result": result, "api": instance.api.snapshot()}),
        }
        .encode()?;
        let mut bytes = bytes.into_boxed_slice();
        let owned = AbiOwnedBytes {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len() as u64,
        };
        std::mem::forget(bytes);
        unsafe {
            output.write(owned);
        }
        Ok(())
    })
}

/// # Safety
/// `bytes` must have been returned by [`call`] exactly once.
pub unsafe fn free_bytes(bytes: AbiOwnedBytes) {
    if !bytes.ptr.is_null() {
        unsafe {
            drop(Vec::from_raw_parts(
                bytes.ptr,
                bytes.len as usize,
                bytes.len as usize,
            ));
        }
    }
}

/// # Safety
/// `handle` must have been returned by [`create`] exactly once.
pub unsafe fn destroy<E: Extension>(handle: AbiHandle) -> AbiStatus {
    guarded(|| {
        if handle.kind != ABI_HANDLE_EXTENSION || handle.raw == 0 {
            return Err("invalid extension handle".into());
        }
        let mut instance = unsafe { Box::from_raw(handle.raw as *mut Instance<E>) };
        instance.extension.shutdown(&mut instance.api)
    })
}

unsafe fn input_json(input: *const AbiBytes) -> Result<JsonEnvelope, String> {
    if input.is_null() {
        return Err("null extension input".into());
    }
    let input = unsafe { &*input };
    if input.len > crate::extensions::MAX_JSON_BYTES as u64
        || (input.len != 0 && input.ptr.is_null())
    {
        return Err("invalid extension input".into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(input.ptr, input.len as usize) };
    JsonEnvelope::parse(bytes)
}

fn guarded(operation: impl FnOnce() -> Result<(), String>) -> AbiStatus {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => ABI_OK,
        Ok(Err(_)) | Err(_) => 1,
    }
}

/// Exports ABI v1 entry points for an [`Extension`] implementation.
#[macro_export]
macro_rules! export_extension {
    ($extension:ty) => {
        extern "C" fn zedflow_create(
            input: *const $crate::extensions::AbiBytes,
            output: *mut $crate::extensions::AbiHandle,
        ) -> $crate::extensions::AbiStatus {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                $crate::sdk::create::<$extension>(input, output)
            }))
            .unwrap_or(1)
        }
        extern "C" fn zedflow_call(
            handle: $crate::extensions::AbiHandle,
            input: *const $crate::extensions::AbiBytes,
            output: *mut $crate::extensions::AbiOwnedBytes,
        ) -> $crate::extensions::AbiStatus {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                $crate::sdk::call::<$extension>(handle, input, output)
            }))
            .unwrap_or(1)
        }
        extern "C" fn zedflow_free_bytes(bytes: $crate::extensions::AbiOwnedBytes) {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                $crate::sdk::free_bytes(bytes)
            }));
        }
        extern "C" fn zedflow_destroy(
            handle: $crate::extensions::AbiHandle,
        ) -> $crate::extensions::AbiStatus {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                $crate::sdk::destroy::<$extension>(handle)
            }))
            .unwrap_or(1)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn zedflow_extension_abi_v1() -> *const $crate::extensions::AbiV1 {
            static TABLE: $crate::extensions::AbiV1 = $crate::extensions::AbiV1 {
                struct_size: std::mem::size_of::<$crate::extensions::AbiV1>() as u64,
                abi_version: $crate::extensions::ABI_V1,
                create: Some(zedflow_create),
                call: Some(zedflow_call),
                free_bytes: Some(zedflow_free_bytes),
                destroy: Some(zedflow_destroy),
            };
            &TABLE
        }
    };
}
