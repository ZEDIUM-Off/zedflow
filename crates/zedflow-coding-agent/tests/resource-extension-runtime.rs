use zedflow_coding_agent::core::{
    source_info::SourceOrigin, source_info::SourceScope, source_info::create_synthetic_source_info,
};
use zedflow_coding_agent::{
    export_html::ansi_to_html,
    extensions::{ExtensionSource, NativeExtensionInstall, receipt},
    resource_loader::{DefaultResourceLoader, ResourceExtensionPaths},
    skills::{Skill, format_skills_for_prompt},
};

#[test]
fn resource_prompt_and_ansi_conversion_keep_trust_boundaries() {
    let skill = Skill {
        name: "safe-skill".into(),
        description: "Use <only> when needed".into(),
        file_path: "/tmp/SKILL.md".into(),
        base_dir: "/tmp".into(),
        source_info: create_synthetic_source_info(
            "/tmp/SKILL.md",
            "test",
            Some(SourceScope::Temporary),
            Some(SourceOrigin::TopLevel),
            None,
        ),
        disable_model_invocation: false,
    };
    let prompt = format_skills_for_prompt(&[skill]);
    assert!(prompt.contains("Use &lt;only&gt; when needed"));
    assert_eq!(
        ansi_to_html("\x1b[31mred\x1b[0m"),
        "<span style=\"color:#800000\">red</span>"
    );
}

#[test]
fn deferred_jiti_input_does_not_block_receipted_native_extensions() {
    let root = std::env::temp_dir().join(format!(
        "zedflow-resource-extension-runtime-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("source");
    let artifact = root.join("artifact.so");
    std::fs::create_dir_all(root.join(".pi/extensions")).unwrap();
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("lib.rs"), "source").unwrap();
    std::fs::write(&artifact, "artifact").unwrap();
    std::fs::write(
        root.join(".pi/extensions/deferred.ts"),
        "export default () => {};",
    )
    .unwrap();
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
    let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
    loader.extend_resources(ResourceExtensionPaths {
        native_extensions: vec![install],
        ..Default::default()
    });
    loader.reload();

    assert!(loader.get_extensions().extensions.is_empty());
    assert!(
        loader.get_extensions().errors[0]
            .message
            .contains("deferred TypeScript/jiti extension")
    );
    assert_eq!(loader.native_extension_artifacts()[0].path, artifact);
    assert!(loader.native_extension_artifacts()[0].trusted);
    let _ = std::fs::remove_dir_all(root);
}
