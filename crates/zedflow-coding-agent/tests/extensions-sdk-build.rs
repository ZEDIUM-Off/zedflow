use std::{fs, path::PathBuf, process::Command};

use sha2::{Digest, Sha256};
use zedflow_coding_agent::extensions::{
    ABI_V1, JsonEnvelope, NativeExtension, NativeExtensionArtifact,
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
    let mut extension = NativeExtension::load(&artifact, &request).expect("load fixture");
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
    extension.shutdown().expect("shutdown fixture");
    assert!(extension.call(&request).is_err());
}
