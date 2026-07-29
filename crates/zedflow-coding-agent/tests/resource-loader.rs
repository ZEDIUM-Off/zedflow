//! Resource-loader integration coverage.

#[test]
fn failed_extension_reload_keeps_active_extensions() {
    use std::fs;
    use zedflow_coding_agent::resource_loader::DefaultResourceLoader;

    let root = std::env::temp_dir().join(format!("zedflow-resource-loader-{}", std::process::id()));
    let extensions = root.join(".pi/extensions");
    fs::create_dir_all(&extensions).unwrap();
    fs::write(extensions.join("active.rs"), "active").unwrap();
    let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
    loader.reload();
    assert_eq!(loader.get_extensions().extensions[0].name, "active");

    fs::remove_dir_all(&extensions).unwrap();
    fs::write(&extensions, "not a directory").unwrap();
    loader.reload();
    assert_eq!(loader.get_extensions().extensions[0].name, "active");
    assert!(!loader.get_extensions().errors.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reload_rejects_forged_or_substituted_persisted_native_receipts() {
    use zedflow_coding_agent::{
        extensions::{ExtensionSource, NativeExtensionInstall, receipt},
        resource_loader::DefaultResourceLoader,
    };

    for case in ["forged", "receipt", "artifact"] {
        let root = std::env::temp_dir().join(format!(
            "zedflow-resource-loader-registration-{case}-{}",
            std::process::id()
        ));
        let extensions = root.join(".pi/extensions");
        let source = extensions.join("source");
        let artifact = root.join("store/extension");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(source.join("lib.rs"), "source").unwrap();
        std::fs::write(&artifact, "local artifact").unwrap();
        let install = NativeExtensionInstall {
            source_dir: source.clone(),
            artifact: artifact.clone(),
            receipt: receipt(
                &ExtensionSource::Path(source.clone()),
                &source,
                &artifact,
                None,
            )
            .unwrap(),
        };
        let receipt_path = extensions
            .join("native-extension-installs")
            .join(format!("{}.json", install.receipt.source_sha256));
        if case == "forged" {
            // Matching receipt-adjacent metadata is not an authorization.
            std::fs::create_dir_all(receipt_path.parent().unwrap()).unwrap();
            std::fs::write(&receipt_path, serde_json::to_vec(&install).unwrap()).unwrap();
            std::fs::write(
                receipt_path.with_extension("json.registered"),
                serde_json::to_vec(&serde_json::json!({
                    "source": install.receipt.source,
                    "source_dir": install.source_dir,
                    "artifact": install.artifact,
                    "source_sha256": install.receipt.source_sha256,
                    "artifact_sha256": install.receipt.artifact_sha256,
                }))
                .unwrap(),
            )
            .unwrap();
        } else {
            install.persist(&extensions).unwrap();
        }
        match case {
            "forged" => {}
            "receipt" => {
                let mut receipt: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
                receipt["receipt"]["source"] = serde_json::json!("crate:substituted@1.0.0");
                std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
            }
            "artifact" => {
                let mut receipt: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
                receipt["artifact"] = serde_json::json!(root.join("substituted-artifact"));
                std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
            }
            _ => unreachable!(),
        }

        let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
        loader.reload();
        assert!(loader.native_extension_artifacts().is_empty(), "{case}");
        assert!(!loader.get_extensions().errors.is_empty(), "{case}");
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn reload_discovers_and_verifies_persisted_native_extension_receipts() {
    use zedflow_coding_agent::{
        extensions::{ExtensionSource, NativeExtensionInstall, receipt},
        resource_loader::DefaultResourceLoader,
    };

    let root = std::env::temp_dir().join(format!(
        "zedflow-resource-loader-receipt-{}",
        std::process::id()
    ));
    let extension_dir = root.join(".pi/extensions");
    let source = extension_dir.join("source");
    let artifact = root.join("store/extension");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(source.join("lib.rs"), "source").unwrap();
    std::fs::write(&artifact, "local artifact").unwrap();
    let install = NativeExtensionInstall {
        source_dir: source.clone(),
        artifact: artifact.clone(),
        receipt: receipt(
            &ExtensionSource::Path(source.clone()),
            &source,
            &artifact,
            None,
        )
        .unwrap(),
    };
    install.persist(&extension_dir).unwrap();

    let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
    loader.reload();

    assert!(loader.get_extensions().errors.is_empty());
    assert_eq!(loader.native_extension_artifacts().len(), 1);
    assert_eq!(loader.native_extension_artifacts()[0].path, artifact);
    assert!(loader.native_extension_artifacts()[0].trusted);
    let _ = std::fs::remove_dir_all(root);
}
