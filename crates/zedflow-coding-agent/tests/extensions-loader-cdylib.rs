use std::path::PathBuf;

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
