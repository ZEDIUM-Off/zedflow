use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use zedflow_coding_agent::{
    extensions::{
        ABI_V1, ExtensionSource, JsonEnvelope, NativeExtension, NativeExtensionArtifact,
        digest_file, digest_tree, install_source,
    },
    resource_loader::{DefaultResourceLoader, ResourceExtensionPaths},
};

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "zedflow-extension-package-manager-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn installs_a_source_built_extension_with_provenance_and_explicit_trust() {
    let root = temp_dir();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-extension");
    let source_dir = root.join("input");
    fs::create_dir_all(source_dir.join("src")).unwrap();
    fs::copy(fixture.join("Cargo.lock"), source_dir.join("Cargo.lock")).unwrap();
    fs::copy(fixture.join("src/lib.rs"), source_dir.join("src/lib.rs")).unwrap();
    let manifest = fs::read_to_string(fixture.join("Cargo.toml"))
        .unwrap()
        .replace(
            "path = \"../../..\"",
            &format!("path = {:?}", PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
        );
    fs::write(source_dir.join("Cargo.toml"), manifest).unwrap();
    let source = ExtensionSource::Path(source_dir.clone());
    let artifact = PathBuf::from(format!(
        "target/release/{}zedflow_rust_extension_fixture{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    let prebuilt = source_dir.join(&artifact);
    fs::create_dir_all(prebuilt.parent().unwrap()).unwrap();
    fs::write(&prebuilt, "not a built extension").unwrap();
    let install = install_source(
        &source,
        &root.join("source"),
        &root.join("staging"),
        &artifact,
        &root.join("store"),
        Some("previous-artifact".into()),
    )
    .expect("source-only extension installation");
    let installed = &install.artifact;
    let receipt = &install.receipt;

    assert!(installed.starts_with(root.join("store")));
    assert!(installed.is_file());
    assert!(install.source_dir.starts_with(root.join("source")));
    assert_eq!(receipt.source, source.canonical());
    assert_eq!(
        receipt.source_sha256,
        digest_tree(&install.source_dir).unwrap()
    );
    assert_eq!(receipt.artifact_sha256, digest_file(installed).unwrap());
    assert_ne!(receipt.artifact_sha256, digest_file(&prebuilt).unwrap());
    assert_eq!(
        receipt.previous_artifact_sha256.as_deref(),
        Some("previous-artifact")
    );
    assert_ne!(receipt.source_sha256, receipt.artifact_sha256);

    let mut resources = DefaultResourceLoader::new(&root, root.join("agent"));
    resources.extend_resources(ResourceExtensionPaths {
        native_extensions: vec![install.clone()],
        ..Default::default()
    });
    resources.reload();
    assert!(resources.get_extensions().errors.is_empty());
    assert_eq!(&resources.native_extension_artifacts()[0].path, installed);

    let request = JsonEnvelope {
        version: ABI_V1,
        payload: serde_json::Value::Null,
    };
    let untrusted = NativeExtensionArtifact {
        path: installed.clone(),
        sha256: receipt.artifact_sha256.clone(),
        trusted: false,
    };
    assert!(matches!(
        NativeExtension::load(&untrusted, &request),
        Err(error) if error == "native extension artifact is not trusted"
    ));

    let mut extension = NativeExtension::load(
        &NativeExtensionArtifact {
            trusted: true,
            ..untrusted
        },
        &request,
    )
    .expect("load explicitly trusted source-built artifact");
    assert_eq!(
        extension
            .call(&JsonEnvelope {
                version: ABI_V1,
                payload: serde_json::json!({"tool":"fixture-tool"})
            })
            .unwrap()
            .payload["result"]["echo"]["tool"],
        "fixture-tool"
    );
    extension.shutdown().unwrap();
    let _ = fs::remove_dir_all(root);
}
