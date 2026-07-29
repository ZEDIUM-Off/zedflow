//! Stable C ABI used by native extension `cdylib`s.
//!
//! This module intentionally contains no pointer dereferences; that boundary is
//! confined to `loader`.

use serde_json::Value;

pub const ABI_V1: u32 = 1;
pub const MAX_JSON_BYTES: usize = 1024 * 1024;

pub type AbiStatus = i32;
pub const ABI_OK: AbiStatus = 0;
pub const ABI_HANDLE_EXTENSION: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiBytes {
    pub ptr: *const u8,
    pub len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiOwnedBytes {
    pub ptr: *mut u8,
    pub len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiHandle {
    pub kind: u32,
    pub reserved: u32,
    pub raw: u64,
    pub generation: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiTableHeader {
    pub struct_size: u64,
    pub abi_version: u32,
}

/// The v1 function table. Each callback is supplied by the extension; the
/// extension owns output allocation and must free it through `free_bytes`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AbiV1 {
    pub struct_size: u64,
    pub abi_version: u32,
    pub create: Option<extern "C" fn(*const AbiBytes, *mut AbiHandle) -> AbiStatus>,
    pub call: Option<extern "C" fn(AbiHandle, *const AbiBytes, *mut AbiOwnedBytes) -> AbiStatus>,
    pub free_bytes: Option<extern "C" fn(AbiOwnedBytes)>,
    pub destroy: Option<extern "C" fn(AbiHandle) -> AbiStatus>,
}

pub type AbiEntryV1 = unsafe extern "C" fn() -> *const AbiV1;

#[derive(Debug, Clone, PartialEq)]
pub struct JsonEnvelope {
    pub version: u32,
    pub payload: Value,
}

impl JsonEnvelope {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_JSON_BYTES {
            return Err(format!("extension JSON exceeds {MAX_JSON_BYTES} bytes"));
        }
        let envelope: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid extension JSON envelope: {error}"))?;
        if envelope.version != ABI_V1 {
            return Err(format!(
                "unsupported extension JSON version: {}",
                envelope.version
            ));
        }
        Ok(envelope)
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        if self.version != ABI_V1 {
            return Err(format!(
                "unsupported extension JSON version: {}",
                self.version
            ));
        }
        let bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_JSON_BYTES {
            return Err(format!("extension JSON exceeds {MAX_JSON_BYTES} bytes"));
        }
        Ok(bytes)
    }
}

impl serde::Serialize for JsonEnvelope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("JsonEnvelope", 2)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("payload", &self.payload)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for JsonEnvelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            version: u32,
            payload: Value,
        }
        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            version: wire.version,
            payload: wire.payload,
        })
    }
}

pub fn validate_table_header(header: AbiTableHeader) -> Result<(), String> {
    if header.abi_version != ABI_V1 {
        return Err(format!(
            "unsupported extension ABI version: {}",
            header.abi_version
        ));
    }
    if header.struct_size != std::mem::size_of::<AbiV1>() as u64 {
        return Err(format!(
            "invalid extension ABI table size: {}",
            header.struct_size
        ));
    }
    Ok(())
}

pub fn validate_table(table: &AbiV1) -> Result<(), String> {
    validate_table_header(AbiTableHeader {
        struct_size: table.struct_size,
        abi_version: table.abi_version,
    })?;
    if table.create.is_none()
        || table.call.is_none()
        || table.free_bytes.is_none()
        || table.destroy.is_none()
    {
        return Err("extension ABI table contains a null callback".into());
    }
    Ok(())
}

pub fn validate_handle(handle: AbiHandle, generation: u64) -> Result<(), String> {
    if handle.kind != ABI_HANDLE_EXTENSION || handle.raw == 0 || handle.generation != generation {
        return Err("stale or invalid extension handle".into());
    }
    Ok(())
}
