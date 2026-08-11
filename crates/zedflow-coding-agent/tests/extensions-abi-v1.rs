use zedflow_coding_agent::extensions::{
    ABI_OK, ABI_V1, AbiBytes, AbiHandle, AbiOwnedBytes, AbiV1, JsonEnvelope,
    abi::{MAX_JSON_BYTES, validate_table},
};

extern "C" fn create(_: *const AbiBytes, _: *mut AbiHandle) -> i32 {
    ABI_OK
}
extern "C" fn call(_: AbiHandle, _: *const AbiBytes, _: *mut AbiOwnedBytes) -> i32 {
    ABI_OK
}
extern "C" fn free(_: AbiOwnedBytes) {}
extern "C" fn destroy(_: AbiHandle) -> i32 {
    ABI_OK
}

#[test]
fn abi_v1_rejects_wrong_table_and_envelope_versions() {
    let mut table = AbiV1 {
        struct_size: AbiV1::STRUCT_SIZE,
        abi_version: ABI_V1,
        create: Some(create),
        call: Some(call),
        free_bytes: Some(free),
        destroy: Some(destroy),
    };
    assert!(validate_table(&table).is_ok());
    table.struct_size -= 1;
    assert!(validate_table(&table).is_err());
    table.struct_size = AbiV1::STRUCT_SIZE;
    table.abi_version += 1;
    assert!(validate_table(&table).is_err());
    table.abi_version = ABI_V1;
    table.create = None;
    assert!(validate_table(&table).is_err());
    assert!(JsonEnvelope::parse(br#"{"version":2,"payload":null}"#).is_err());
    assert!(JsonEnvelope::parse(&vec![b'x'; MAX_JSON_BYTES + 1]).is_err());
}
