use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, Barrier},
};

use sha2::{Digest, Sha256};
use zedflow_coding_agent::{
    export_extension,
    extensions::{
        ABI_OK, ABI_V1, AbiBytes, AbiHandle, AbiOwnedBytes, ExtensionRunner, JsonEnvelope,
        NativeExtension, NativeExtensionArtifact, create_extension_runtime,
    },
    sdk::{self, Extension, ExtensionApi, JsonValue},
};

#[test]
fn sdk_fixture_builds_and_exercises_abi_v1() {
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-extension/Cargo.toml");
    let target = std::env::temp_dir().join("zedflow-sdk-fixture-target");
    assert!(
        Command::new("cargo")
            .args(["build", "--manifest-path"])
            .arg(&manifest)
            .args(["--target-dir"])
            .arg(&target)
            .status()
            .expect("run cargo build")
            .success()
    );

    let library = target.join("debug").join(format!(
        "{}zedflow_rust_extension_fixture{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    let artifact = NativeExtensionArtifact {
        sha256: format!(
            "{:x}",
            Sha256::digest(fs::read(&library).expect("fixture library"))
        ),
        path: library,
        trusted: true,
    };
    let request = JsonEnvelope {
        version: ABI_V1,
        payload: serde_json::json!({"start":true}),
    };
    let extension = NativeExtension::load(&artifact, &request).expect("load fixture");
    let reply = extension
        .call(&JsonEnvelope {
            version: ABI_V1,
            payload: serde_json::json!({"tool":"fixture-tool"}),
        })
        .expect("invoke fixture");
    assert_eq!(reply.payload["result"]["echo"]["tool"], "fixture-tool");
    assert_eq!(
        reply.payload["api"]["tools"],
        serde_json::json!(["fixture-tool"])
    );
    assert_eq!(
        reply.payload["api"]["providers"]["fixture-provider"]["model"],
        "fixture"
    );
    assert_eq!(reply.payload["api"]["ui"][0]["text"], "fixture UI");
    assert_eq!(
        reply.payload["api"]["events"],
        serde_json::json!(["session_start", "tool_call"])
    );

    std::thread::scope(|scope| {
        let extension = &extension;
        let barrier = Arc::new(Barrier::new(9));
        for _ in 0..8 {
            let barrier = Arc::clone(&barrier);
            scope.spawn(move || {
                barrier.wait();
                extension
                    .call(&JsonEnvelope {
                        version: ABI_V1,
                        payload: serde_json::json!({"parallel": true}),
                    })
                    .expect("serialized concurrent call");
            });
        }
        barrier.wait();
    });
    let final_reply = extension
        .call(&JsonEnvelope {
            version: ABI_V1,
            payload: serde_json::Value::Null,
        })
        .expect("final call");
    assert_eq!(
        final_reply.payload["api"]["events"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
    let mut runtime = create_extension_runtime();
    extension.activate(&mut runtime).expect("activate fixture");
    assert_eq!(runtime.tools[0].name, "fixture-tool");
    assert_eq!(runtime.commands[0].name, "fixture-command");
    assert_eq!(runtime.providers[0].name, "fixture-provider");

    let mut runner = ExtensionRunner::with_runtime(vec![], runtime);
    runner.set_context(
        zedflow_coding_agent::extensions::ExtensionMode::Tui,
        "/work",
        true,
    );
    assert_eq!(
        runner
            .invoke_tool("fixture-tool", serde_json::json!({"answer": 42}))
            .expect("invoke activated fixture tool"),
        serde_json::json!({
            "echo": {
                "arguments": {"answer": 42},
                "context": {"cwd": "/work", "generation": 0, "hasUi": true},
                "kind": "tool",
                "name": "fixture-tool"
            }
        })
    );
    assert!(
        !runner
            .invoke_command("fixture-command", &["run".into()])
            .expect("invoke activated fixture command")
            .cancelled
    );
}

#[derive(Default)]
struct CreatePanics;

impl Extension for CreatePanics {
    fn initialize(&mut self, _: &mut ExtensionApi, _: JsonValue) -> Result<(), String> {
        panic!("extension panic")
    }
}

#[derive(Default)]
struct PanickingExtension;

impl Extension for PanickingExtension {
    fn invoke(&mut self, _: &mut ExtensionApi, _: JsonValue) -> Result<JsonValue, String> {
        panic!("extension panic")
    }

    fn shutdown(&mut self, _: &mut ExtensionApi) -> Result<(), String> {
        panic!("extension panic")
    }
}

export_extension!(PanickingExtension);

#[test]
fn sdk_abi_trampolines_contain_extension_panics() {
    let request = JsonEnvelope {
        version: ABI_V1,
        payload: serde_json::Value::Null,
    }
    .encode()
    .unwrap();
    let input = AbiBytes {
        ptr: request.as_ptr(),
        len: request.len() as u64,
    };
    let mut failed_handle = AbiHandle {
        kind: 0,
        reserved: 0,
        raw: 0,
        generation: 0,
    };
    assert_ne!(
        unsafe { sdk::create::<CreatePanics>(&input, &mut failed_handle) },
        ABI_OK
    );
    assert_eq!(failed_handle.raw, 0);

    let table = unsafe { &*zedflow_extension_abi_v1() };
    let mut handle = AbiHandle {
        kind: 0,
        reserved: 0,
        raw: 0,
        generation: 0,
    };
    assert_eq!(table.create.unwrap()(&input, &mut handle), ABI_OK);

    let mut output = AbiOwnedBytes {
        ptr: std::ptr::null_mut(),
        len: 0,
    };
    assert_ne!(table.call.unwrap()(handle, &input, &mut output), ABI_OK);
    assert!(output.ptr.is_null());
    assert_ne!(table.destroy.unwrap()(handle), ABI_OK);
}
