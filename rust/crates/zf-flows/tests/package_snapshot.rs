use std::collections::BTreeMap;
use zf_flows::package::{
    FlowPackageManifest, MAX_MANIFEST_BYTES, MAX_PACKAGE_FILE_BYTES, MAX_PACKAGE_FILES,
    MAX_SNAPSHOT_PACKAGES, PackageNode, PackageSnapshot,
};

fn capture(
    id: &str,
    files: &[(&str, &[u8])],
    dependencies: BTreeMap<String, PackageSnapshot>,
) -> PackageSnapshot {
    let manifest = serde_json::json!({
        "formatVersion": 1,
        "id": id,
        "name": "shared-cargo-name",
        "description": "Métadonnées exactes",
        "entry": "flow.rs",
        "files": files.iter().map(|(path, _)| path).collect::<Vec<_>>(),
        "dependencies": dependencies.keys().map(|alias| (alias.clone(), serde_json::json!({"path": format!("../{alias}")}))).collect::<BTreeMap<_, _>>()
    });
    PackageSnapshot::capture(
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
        files
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
            .collect(),
        dependencies,
    )
    .unwrap()
}

fn leaf(id: &str, secondary: &[u8]) -> PackageSnapshot {
    capture(
        id,
        &[
            ("flow.rs", b"// flow source\n"),
            ("assets/étoile.bin", secondary),
        ],
        BTreeMap::new(),
    )
}

#[test]
fn secondary_file_and_dependency_edits_change_distinct_revisions() {
    let first = leaf("dependency", b"first");
    let second = leaf("dependency", b"second");
    assert_ne!(first.root, second.root);
    assert_ne!(
        first.root_node().unwrap().content_revision(),
        second.root_node().unwrap().content_revision()
    );
    assert_eq!(
        first.root_node().unwrap().dependencies_revision(),
        second.root_node().unwrap().dependencies_revision()
    );
    let root_first = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("child".into(), first)]),
    );
    let root_second = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("child".into(), second)]),
    );
    assert_eq!(
        root_first.root_node().unwrap().content_revision(),
        root_second.root_node().unwrap().content_revision()
    );
    assert_ne!(
        root_first.root_node().unwrap().dependencies_revision(),
        root_second.root_node().unwrap().dependencies_revision()
    );
    assert_ne!(root_first.root, root_second.root);
}

#[test]
fn exact_manifest_and_binary_unicode_inventory_round_trip() {
    let snapshot = leaf("étoile", &[0, 255, 13, 10, 0, 128]);
    let archive = serde_json::to_vec(&snapshot).unwrap();
    let restored: PackageSnapshot = serde_json::from_slice(&archive).unwrap();
    restored.validate().unwrap();
    assert_eq!(snapshot, restored);
    let node = restored.root_node().unwrap();
    assert!(node.manifest_source.ends_with('\n'));
    assert_eq!(node.entry_source().unwrap(), "// flow source\n");
    assert_eq!(node.files["assets/étoile.bin"], [0, 255, 13, 10, 0, 128]);
    assert_eq!(node.package_id().unwrap().as_str(), "étoile");
    let manifest = FlowPackageManifest::parse(&node.manifest_source).unwrap();
    assert_eq!(manifest.description.as_deref(), Some("Métadonnées exactes"));
    assert_eq!(
        serde_json::from_str::<FlowPackageManifest>(&serde_json::to_string(&manifest).unwrap())
            .unwrap(),
        manifest
    );
}

#[test]
fn map_order_is_canonical_but_manifest_source_bytes_are_not_normalized() {
    let snapshot = leaf("flow", b"asset");
    let original = snapshot.root_node().unwrap();
    let mut reordered = original.clone();
    reordered.files = original
        .files
        .iter()
        .rev()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    assert_eq!(original.revision(), reordered.revision());
    reordered.manifest_source.push('\n');
    assert_eq!(original.manifest().unwrap(), reordered.manifest().unwrap());
    assert_ne!(original.content_revision(), reordered.content_revision());
    let mut first = original.clone();
    first.dependencies =
        BTreeMap::from([("z".into(), "a".repeat(64)), ("a".into(), "b".repeat(64))]);
    let mut second = first.clone();
    second.dependencies = first
        .dependencies
        .iter()
        .rev()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    assert_eq!(
        first.dependencies_revision(),
        second.dependencies_revision()
    );
    assert_ne!(
        original.content_revision(),
        original.dependencies_revision()
    );
    assert_ne!(original.content_revision(), original.revision());
}

#[test]
fn diamond_closure_deduplicates_shared_node_and_allows_equal_cargo_names() {
    let shared = leaf("shared", b"shared");
    let left = capture(
        "left",
        &[("flow.rs", b"// left")],
        BTreeMap::from([("shared".into(), shared.clone())]),
    );
    let right = capture(
        "right",
        &[("flow.rs", b"// right")],
        BTreeMap::from([("shared".into(), shared.clone())]),
    );
    let root = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("left".into(), left), ("right".into(), right)]),
    );
    assert_eq!(root.packages.len(), 4);
    assert!(root.packages.contains_key(&shared.root));
    root.validate().unwrap();
    assert!(
        root.packages
            .values()
            .all(|node| node.manifest().unwrap().name == "shared-cargo-name")
    );
}

#[test]
fn conflicting_revisions_for_one_flow_identity_are_rejected() {
    let left = leaf("same-id", b"left");
    let right = leaf("same-id", b"right");
    let template = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([
            ("left".into(), left.clone()),
            ("right".into(), leaf("another-id", b"right")),
        ]),
    );
    let node = template.root_node().unwrap();
    let error = PackageSnapshot::capture(
        node.manifest_source.clone(),
        node.files.clone(),
        BTreeMap::from([("left".into(), left), ("right".into(), right)]),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("conflicting revisions for flow identity")
    );
}

#[test]
fn archive_corruption_missing_nodes_and_orphans_are_rejected() {
    let snapshot = leaf("root", b"original");
    let mut corrupt = snapshot.clone();
    corrupt
        .packages
        .get_mut(&snapshot.root)
        .unwrap()
        .files
        .get_mut("assets/étoile.bin")
        .unwrap()
        .push(1);
    assert!(
        corrupt
            .validate()
            .unwrap_err()
            .to_string()
            .contains("integrity mismatch")
    );
    let mut orphan = snapshot.clone();
    orphan.packages.extend(leaf("orphan", b"extra").packages);
    assert!(
        orphan
            .validate()
            .unwrap_err()
            .to_string()
            .contains("orphan")
    );
    let mut missing = capture(
        "parent",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("child".into(), snapshot)]),
    );
    missing
        .packages
        .retain(|revision, _| revision == &missing.root);
    assert!(
        missing
            .validate()
            .unwrap_err()
            .to_string()
            .contains("missing dependency")
    );
}

#[test]
fn malformed_hash_and_cycle_are_rejected_without_recursive_traversal() {
    let mut malformed = leaf("root", b"asset");
    malformed.root = "BAD-HASH".into();
    assert!(
        malformed
            .validate()
            .unwrap_err()
            .to_string()
            .contains("invalid package revision hash")
    );
    let mut cycle = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("self".into(), leaf("child", b"asset"))]),
    );
    cycle.packages.retain(|revision, _| revision == &cycle.root);
    cycle
        .packages
        .get_mut(&cycle.root)
        .unwrap()
        .dependencies
        .insert("self".into(), cycle.root.clone());
    assert!(cycle.validate().unwrap_err().to_string().contains("cyclic"));
}

#[test]
fn inventory_aliases_entry_encoding_and_file_directory_collisions_are_checked() {
    let snapshot = leaf("root", b"asset");
    let original = snapshot.root_node().unwrap();
    let mut extra = original.files.clone();
    extra.insert("undeclared.rs".into(), b"// extra".to_vec());
    assert!(
        PackageSnapshot::capture(original.manifest_source.clone(), extra, BTreeMap::new()).is_err()
    );
    let mut missing = original.files.clone();
    missing.remove("assets/étoile.bin");
    assert!(
        PackageSnapshot::capture(original.manifest_source.clone(), missing, BTreeMap::new())
            .is_err()
    );
    assert!(
        PackageSnapshot::capture(
            original.manifest_source.clone(),
            original.files.clone(),
            BTreeMap::from([("extra".into(), leaf("extra", b"asset"))])
        )
        .is_err()
    );
    let mut invalid_utf8 = original.files.clone();
    invalid_utf8.insert("flow.rs".into(), vec![255]);
    assert!(
        PackageSnapshot::capture(
            original.manifest_source.clone(),
            invalid_utf8,
            BTreeMap::new()
        )
        .is_err()
    );
    let mut manifest = original.manifest().unwrap();
    manifest.files.extend(["assets".into()]);
    assert!(
        manifest
            .validate()
            .unwrap_err()
            .to_string()
            .contains("also a directory")
    );
}

#[test]
fn snapshot_and_node_unknown_fields_are_rejected_and_old_metadata_stays_readable() {
    let old_manifest = r#"{"formatVersion":1,"id":"historical","name":"name","entry":"flow.rs","files":["flow.rs"]}"#;
    let manifest = FlowPackageManifest::parse(old_manifest).unwrap();
    assert_eq!(manifest.description, None);
    assert!(manifest.dependencies.is_empty());
    let snapshot = PackageSnapshot::capture(
        old_manifest.into(),
        BTreeMap::from([("flow.rs".into(), b"// old".to_vec())]),
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(snapshot.root_node().unwrap().manifest_source, old_manifest);
    // Fixed byte-level vectors, independent of the archive's serde file codec.
    assert_eq!(
        snapshot.root_node().unwrap().content_revision(),
        "52861a6e437a5686614b6ae73fc0aad13ba0fe59e93a37a44b1693a53d700926"
    );
    assert_eq!(
        snapshot.root_node().unwrap().dependencies_revision(),
        "a8e2ad0a34965621644195d180b0f643654132419eff656b27182df71adf788b"
    );
    assert_eq!(
        snapshot.root,
        "fea998d7fd6cf01257633953074f690469a4103eb73ba3fd908ef3dd3c28ca73"
    );
    let mut value = serde_json::to_value(&snapshot).unwrap();
    value["surprise"] = true.into();
    assert!(serde_json::from_value::<PackageSnapshot>(value).is_err());
    let mut node = serde_json::to_value(snapshot.root_node().unwrap()).unwrap();
    node["surprise"] = true.into();
    assert!(serde_json::from_value::<PackageNode>(node).is_err());
}

#[test]
fn payload_limits_reject_oversize_file_manifest_and_combined_closure() {
    let mut oversized_file = leaf("root", b"asset");
    oversized_file
        .packages
        .get_mut(&oversized_file.root)
        .unwrap()
        .files
        .insert(
            "assets/étoile.bin".into(),
            vec![0; MAX_PACKAGE_FILE_BYTES + 1],
        );
    let error = oversized_file.validate().unwrap_err();
    assert!(format!("{error:#}").contains("file exceeds 16 MiB"));
    drop(oversized_file);

    let mut manifest = leaf("root", b"asset");
    manifest
        .packages
        .get_mut(&manifest.root)
        .unwrap()
        .manifest_source = " ".repeat(MAX_MANIFEST_BYTES + 1);
    assert!(format!("{:#}", manifest.validate().unwrap_err()).contains("manifest exceeds 1 MiB"));

    let files: &[(&str, &[u8])] = &[
        ("flow.rs", b"// flow"),
        ("a.bin", b"a"),
        ("b.bin", b"b"),
        ("c.bin", b"c"),
    ];
    let child = capture("child", files, BTreeMap::new());
    let mut combined = capture("root", files, BTreeMap::from([("child".into(), child)]));
    // Each node is individually below the total bound and every file is exactly
    // at its bound. The two nodes plus their manifests exceed the closure bound.
    for node in combined.packages.values_mut() {
        for bytes in node.files.values_mut() {
            *bytes = vec![0; MAX_PACKAGE_FILE_BYTES];
        }
    }
    assert!(format!("{:#}", combined.validate().unwrap_err()).contains("snapshot exceeds 128 MiB"));
}

#[test]
fn package_and_inventory_counts_are_bounded_before_traversal() {
    let mut too_many = leaf("root", b"asset");
    let node = too_many.root_node().unwrap().clone();
    for index in 0..MAX_SNAPSHOT_PACKAGES {
        too_many
            .packages
            .insert(format!("{index:064x}"), node.clone());
    }
    assert!(
        too_many
            .validate()
            .unwrap_err()
            .to_string()
            .contains("too many snapshot packages")
    );
    let mut too_many_files = leaf("root", b"asset");
    let files = &mut too_many_files
        .packages
        .get_mut(&too_many_files.root)
        .unwrap()
        .files;
    for index in 0..MAX_PACKAGE_FILES {
        files.insert(format!("asset-{index}"), Vec::new());
    }
    assert!(
        format!("{:#}", too_many_files.validate().unwrap_err()).contains("too many package files")
    );
}

#[test]
fn dependency_and_node_hashes_require_exact_lowercase_sha256() {
    for invalid in [
        "a".repeat(63),
        "b".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        "é".repeat(32),
    ] {
        let mut key = leaf("root", b"asset");
        let node = key.packages.remove(&key.root).unwrap();
        key.root.clone_from(&invalid);
        key.packages.insert(invalid.clone(), node);
        assert!(
            key.validate()
                .unwrap_err()
                .to_string()
                .contains("invalid package revision hash")
        );
        let mut dependency = capture(
            "root",
            &[("flow.rs", b"// flow")],
            BTreeMap::from([("child".into(), leaf("child", b"asset"))]),
        );
        dependency
            .packages
            .get_mut(&dependency.root)
            .unwrap()
            .dependencies
            .insert("child".into(), invalid);
        assert!(
            dependency
                .validate()
                .unwrap_err()
                .to_string()
                .contains("invalid package revision hash")
        );
    }
}

#[test]
fn multi_node_cycle_is_diagnosed_before_untrusted_hash_contents() {
    let child = capture(
        "child",
        &[("flow.rs", b"// child")],
        BTreeMap::from([("next".into(), leaf("grandchild", b"asset"))]),
    );
    let child_revision = child.root.clone();
    let mut root = capture(
        "root",
        &[("flow.rs", b"// root")],
        BTreeMap::from([("next".into(), child)]),
    );
    root.packages
        .retain(|revision, _| revision == &root.root || revision == &child_revision);
    root.packages
        .get_mut(&child_revision)
        .unwrap()
        .dependencies
        .insert("next".into(), root.root.clone());
    assert!(
        root.validate()
            .unwrap_err()
            .to_string()
            .contains("cyclic package dependency closure")
    );
}

#[test]
fn traversal_reserved_files_and_ambiguous_inventory_paths_are_rejected() {
    let manifest = leaf("root", b"asset").root_manifest().unwrap();
    for path in [
        "../outside",
        "/absolute",
        "a/../outside",
        "a//b",
        "a/./b",
        "a\\b",
        "C:/file",
        "flow.json",
        "target/output",
        "a/.git/config",
        "a/.env",
        "a/.env.private",
        "node_modules/a",
        "a/build.rs",
        "flow.rs/child",
        "control\ncharacter",
    ] {
        let mut invalid = manifest.clone();
        invalid.files.push(path.into());
        assert!(
            invalid.validate().is_err(),
            "accepted ambiguous path {path:?}"
        );
    }
}

#[test]
fn archive_reuses_exact_text_values_and_encodes_only_non_utf8_bytes() {
    let source = "// Été 🦀\r\n\t // no normalization\n";
    let snapshot = capture(
        "root",
        &[
            ("flow.rs", source.as_bytes()),
            ("empty.txt", b""),
            ("binary.bin", &[255, 0]),
        ],
        BTreeMap::new(),
    );
    let archive = serde_json::to_value(&snapshot).unwrap();
    let files = &archive["packages"][&snapshot.root]["files"];
    assert_eq!(files["flow.rs"], source);
    assert_eq!(files["empty.txt"], "");
    assert_eq!(files["binary.bin"], serde_json::json!({"base64": "/wA="}));
    let restored: PackageSnapshot = serde_json::from_value(archive).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, snapshot);
    assert_eq!(
        restored.root_node().unwrap().entry_source().unwrap(),
        source
    );
}

#[test]
fn archive_binary_objects_are_strict_and_corrupted_bytes_fail_integrity() {
    let snapshot = leaf("root", &[255, 0]);
    for invalid in [
        serde_json::json!([255, 0]),
        serde_json::json!({}),
        serde_json::json!({"base64": "/wA=", "extra": true}),
        serde_json::json!({"base64": false}),
        serde_json::json!({"base64": "!!!!"}),
        serde_json::json!({"base64": "/wA"}),
        serde_json::json!({"base64": "/wB="}),
        serde_json::json!({"base64": "/wA=\n"}),
        serde_json::json!({"base64": ""}),
        serde_json::json!({"base64": "dGV4dA=="}),
    ] {
        let mut archive = serde_json::to_value(&snapshot).unwrap();
        archive["packages"][&snapshot.root]["files"]["assets/étoile.bin"] = invalid;
        assert!(serde_json::from_value::<PackageSnapshot>(archive).is_err());
    }
    let mut corrupt = serde_json::to_value(&snapshot).unwrap();
    corrupt["packages"][&snapshot.root]["files"]["assets/étoile.bin"] =
        serde_json::json!({"base64": "/gA="});
    let decoded: PackageSnapshot = serde_json::from_value(corrupt).unwrap();
    assert!(
        decoded
            .validate()
            .unwrap_err()
            .to_string()
            .contains("integrity mismatch")
    );

    let duplicate_object = r#"{"manifestSource":"","files":{"x":{"base64":"/w==","base64":"/g=="}},"dependencies":{}}"#;
    assert!(serde_json::from_str::<PackageNode>(duplicate_object).is_err());
    let duplicate_path =
        r#"{"manifestSource":"","files":{"x":"first","x":"second"},"dependencies":{}}"#;
    assert!(serde_json::from_str::<PackageNode>(duplicate_path).is_err());
}

#[test]
fn file_codec_rejects_oversized_text_and_encoded_binary_before_decoding() {
    let text = "x".repeat(MAX_PACKAGE_FILE_BYTES + 1);
    let node = serde_json::json!({"manifestSource": "", "files": {"x": text}, "dependencies": {}});
    assert!(
        serde_json::from_value::<PackageNode>(node)
            .unwrap_err()
            .to_string()
            .contains("file exceeds 16 MiB")
    );
    let base64 = "A".repeat(MAX_PACKAGE_FILE_BYTES.div_ceil(3) * 4 + 4);
    let node = serde_json::json!({"manifestSource": "", "files": {"x": {"base64": base64}}, "dependencies": {}});
    assert!(
        serde_json::from_value::<PackageNode>(node)
            .unwrap_err()
            .to_string()
            .contains("file exceeds 16 MiB")
    );
}

#[test]
fn file_codec_enforces_cumulative_decoded_inventory_limit() {
    let mut files = serde_json::Map::new();
    for index in 0..8 {
        files.insert(
            format!("a{index}"),
            "x".repeat(MAX_PACKAGE_FILE_BYTES).into(),
        );
    }
    files.insert("z.bin".into(), serde_json::json!({"base64": "/w=="}));
    let node = serde_json::json!({"manifestSource": "", "files": files, "dependencies": {}});
    assert!(
        serde_json::from_value::<PackageNode>(node)
            .unwrap_err()
            .to_string()
            .contains("inventory exceeds 128 MiB")
    );
}
