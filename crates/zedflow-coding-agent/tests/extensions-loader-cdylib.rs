use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use zedflow_coding_agent::extensions::{
    ABI_V1, JsonEnvelope, NativeExtension, NativeExtensionArtifact,
};

#[test]
fn native_loader_requires_explicit_trust_before_touching_artifact() {
    let artifact = NativeExtensionArtifact {
        path: PathBuf::from("does-not-exist.so"),
        sha256: "00".repeat(32),
        trusted: false,
    };
    let request = JsonEnvelope {
        version: ABI_V1,
        payload: serde_json::Value::Null,
    };
    assert!(matches!(
        NativeExtension::load(&artifact, &request),
        Err(error) if error == "native extension artifact is not trusted"
    ));
}

#[test]
fn native_loader_rejects_a_swapped_artifact() {
    let path = std::env::temp_dir().join(format!(
        "zedflow-loader-swap-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let original = b"trusted artifact";
    fs::write(&path, original).unwrap();
    let artifact = NativeExtensionArtifact {
        path: path.clone(),
        sha256: format!("{:x}", Sha256::digest(original)),
        trusted: true,
    };
    fs::write(&path, b"swapped artifact").unwrap();

    let request = JsonEnvelope {
        version: ABI_V1,
        payload: serde_json::Value::Null,
    };
    assert!(matches!(
        NativeExtension::load(&artifact, &request),
        Err(error) if error == "native extension artifact SHA-256 mismatch"
    ));
    fs::remove_file(path).unwrap();
}
