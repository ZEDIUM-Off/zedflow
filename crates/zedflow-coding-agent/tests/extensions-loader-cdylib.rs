use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use zedflow_coding_agent::{
    extensions::{
        ABI_V1, ExtensionSource, JsonEnvelope, NativeExtension, NativeExtensionArtifact,
        NativeExtensionInstall, receipt,
    },
    resource_loader::{DefaultResourceLoader, ResourceExtensionPaths},
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
fn resource_loader_resolves_only_receipted_native_artifacts() {
    let root = std::env::temp_dir().join(format!(
        "zedflow-loader-receipt-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("source");
    let artifact = root.join("artifact.so");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("Cargo.toml"), "[package]\nname='native'\n").unwrap();
    fs::write(&artifact, b"native artifact").unwrap();
    let receipt = receipt(
        &ExtensionSource::Path(source.clone()),
        &source,
        &artifact,
        None,
    )
    .unwrap();
    let mut resources = DefaultResourceLoader::new(&root, root.join("agent"));
    resources.extend_resources(ResourceExtensionPaths {
        native_extensions: vec![NativeExtensionInstall {
            source_dir: source,
            artifact: artifact.clone(),
            receipt,
        }],
        ..Default::default()
    });
    resources.reload();
    assert!(resources.get_extensions().errors.is_empty());
    assert_eq!(resources.native_extension_artifacts().len(), 1);
    assert!(resources.native_extension_artifacts()[0].trusted);
    assert_eq!(resources.native_extension_artifacts()[0].path, artifact);
    fs::remove_dir_all(root).unwrap();
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
