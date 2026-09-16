use std::collections::BTreeMap;
use zf_compiler::package_sources::validate_package_sources;
use zf_flows::package::PackageSnapshot;

fn package(files: &[(&str, &[u8])], dependency: Option<PackageSnapshot>) -> PackageSnapshot {
    let dependencies = dependency
        .map(|dependency| BTreeMap::from([("local".into(), dependency)]))
        .unwrap_or_default();
    let id = if dependencies.is_empty() {
        "leaf"
    } else {
        "root"
    };
    let manifest = serde_json::json!({
        "formatVersion": 1, "id": id, "name": id, "entry": "flow.rs",
        "files": files.iter().map(|(path, _)| path).collect::<Vec<_>>(),
        "dependencies": if dependencies.is_empty() { serde_json::json!({}) }
            else { serde_json::json!({"local": {"path": "../local"}}) },
    });
    PackageSnapshot::capture(
        manifest.to_string(),
        files
            .iter()
            .map(|(path, bytes)| ((*path).into(), bytes.to_vec()))
            .collect(),
        dependencies,
    )
    .unwrap()
}

fn source_package(source: &str) -> PackageSnapshot {
    package(
        &[
            ("flow.rs", b"fn flow() {}"),
            ("secondary.rs", source.as_bytes()),
        ],
        None,
    )
}

#[test]
fn rejects_absolute_traversing_undeclared_and_dynamic_references_in_unused_sources() {
    for source in [
        r#"const A: &str = include_str!("/tmp/outside.txt");"#,
        r#"const A: &[u8] = std::include_bytes!(r"C:\outside.txt");"#,
        r#"include!("../outside.rs");"#,
        r#"include_str!("missing.txt");"#,
        r#"include_str!(concat!("assets/", "name.txt"));"#,
        r#"include_bytes!(env!("OUT_DIR"));"#,
        r#"include_str!(r"\\server\outside.txt");"#,
        r#"include_str!("\x2ftmp/outside.txt");"#,
        r#"include_str!("sub/../outside.txt");"#,
    ] {
        let snapshot = source_package(source);
        snapshot.validate().unwrap(); // Acquisition stays byte-agnostic.
        let error = format!("{:#}", validate_package_sources(&snapshot).unwrap_err());
        assert!(error.contains("secondary.rs"), "{source}: {error}");
    }
}

#[test]
fn accepts_declared_assets_without_interpreting_runtime_paths_comments_or_strings() {
    let snapshot = package(
        &[
            (
                "flow.rs",
                br####"
            // include_str!("/tmp/comment")
            /* #[path = "/tmp/comment.rs"] mod ignored; */
            fn flow() { let _ = std::fs::read_to_string("/runtime/tool/path"); }
            const TEXT: &str = r###"include!("/tmp/string.rs"); mod missing;"###;
            const ASSET: &str = include_str!(r#"./assets/a.txt"#,);
            const BYTES: &[u8] = ::std::include_bytes! { "assets/b.bin" };
        "####,
            ),
            ("assets/a.txt", b"text"),
            ("assets/b.bin", &[0, 255, 128]),
        ],
        None,
    );
    validate_package_sources(&snapshot).unwrap();
}

#[test]
fn validates_all_dependency_sources_even_when_root_never_imports_them() {
    let dependency = source_package(r#"include_str!("/tmp/dependency-secret");"#);
    let snapshot = package(&[("flow.rs", b"fn flow() {}")], Some(dependency));
    let error = format!("{:#}", validate_package_sources(&snapshot).unwrap_err());
    assert!(error.contains("package leaf"), "{error}");
    assert!(error.contains("secondary.rs"), "{error}");
    let dependency = package(
        &[
            ("flow.rs", b"mod helper;"),
            ("helper.rs", br#"include_str!("asset.txt");"#),
            ("asset.txt", b"frozen"),
        ],
        None,
    );
    validate_package_sources(&package(&[("flow.rs", b"fn flow() {}")], Some(dependency))).unwrap();
}

#[test]
fn preserves_nested_default_inline_and_explicit_module_resolution() {
    let snapshot = package(&[
        ("flow.rs", br#"mod flat; mod tree; #[path = "other/renamed.rs"] mod alias; mod inline { mod child; } mod lib;"#),
        ("flat.rs", br#"mod child; mod inline { #[path = "leaf.rs"] mod selected; }"#),
        ("flat/child.rs", b"fn child() {}"),
        ("flat/inline/leaf.rs", b"fn leaf() {}"),
        ("tree/mod.rs", b"mod child;"),
        ("tree/child.rs", b"fn child() {}"),
        // Explicit path modules resolve children relative to the referenced file's
        // parent, rather than its filename or the declaration's alias.
        ("other/renamed.rs", br#"mod sibling; mod inline { mod leaf; }"#),
        ("other/sibling.rs", b"fn sibling() {}"),
        ("other/inline/leaf.rs", b"fn leaf() {}"),
        ("inline/child.rs", b"fn child() {}"),
        ("lib.rs", b"mod nested;"),
        ("lib/nested.rs", b"fn nested() {}"),
    ], None);
    validate_package_sources(&snapshot).unwrap();
}

#[test]
fn explicit_paths_in_inline_modules_select_declared_directories() {
    let snapshot = package(&[
        ("flow.rs", br#"#[path = "alternate"] mod inline { mod child; #[path = "selected.rs"] mod leaf; }"#),
        ("alternate/child.rs", b"fn child() {}"),
        ("alternate/selected.rs", b"fn leaf() {}"),
    ], None);
    validate_package_sources(&snapshot).unwrap();
}

#[test]
fn rejects_external_module_escape_missing_ambiguity_and_dynamic_paths() {
    for source in [
        "mod missing;",
        r#"#[path = "/tmp/external.rs"] mod outside;"#,
        r#"#[path = "../external.rs"] mod outside;"#,
        r#"#[path = concat!("other", ".rs")] mod outside;"#,
        r#"#[cfg_attr(any(), path = "/tmp/external.rs")] mod outside;"#,
        r#"#[path = "missing.rs"] mod outside;"#,
        r#"#[path = "/tmp"] mod inline { mod outside; }"#,
    ] {
        assert!(
            validate_package_sources(&source_package(source)).is_err(),
            "{source}"
        );
    }
    let snapshot = package(
        &[
            ("flow.rs", b"mod ambiguous;"),
            ("ambiguous.rs", b""),
            ("ambiguous/mod.rs", b""),
        ],
        None,
    );
    let error = format!("{:#}", validate_package_sources(&snapshot).unwrap_err());
    assert!(error.contains("exactly one"), "{error}");
}

#[test]
fn inspects_disabled_code_attribute_macros_and_nested_macro_arguments() {
    for source in [
        r#"#[cfg(any())] mod ignored { const A: &str = include_str!("/tmp/secret"); }"#,
        r#"#[doc = include_str!("/tmp/secret")] fn documented() {}"#,
        r#"some_macro! { nested!(include_bytes!("/tmp/secret")) }"#,
        r#"macro_rules! hidden { () => { include!("/tmp/secret") } }"#,
        r#"macro_rules! dynamic { ($path:expr) => { include_str!($path) } }"#,
        r#"macro_rules! module { () => { mod child; } }"#,
        r#"use std::include_str as renamed; renamed!("/tmp/secret");"#,
        r#"forward!(include_str, "/tmp/secret");"#,
        r#"use core::arch::{global_asm as load}; load!(".incbin /tmp/secret");"#,
        r#"core::arch::global_asm!(".incbin \"/tmp/secret\"");"#,
    ] {
        assert!(
            validate_package_sources(&source_package(source)).is_err(),
            "{source}"
        );
    }
}

#[test]
fn resolves_configuration_path_alternatives_without_evaluating_host_configuration() {
    let snapshot = package(
        &[
            (
                "flow.rs",
                br#"#[cfg_attr(feature = "other", path = "alternate.rs")] mod regular;"#,
            ),
            ("regular.rs", b"mod nested;"),
            ("regular/nested.rs", b""),
            ("alternate.rs", b"mod sibling;"),
            ("sibling.rs", b""),
        ],
        None,
    );
    validate_package_sources(&snapshot).unwrap();
}

#[test]
fn follows_rust_includes_and_path_modules_even_without_rs_extension() {
    let valid = package(
        &[
            (
                "flow.rs",
                br#"include!("code/items.inc"); #[path = "code/other.inc"] mod other;"#,
            ),
            (
                "code/items.inc",
                br#"mod child; const DATA: &str = include_str!("asset.txt");"#,
            ),
            ("code/child.rs", b""),
            ("code/asset.txt", b"frozen"),
            ("code/other.inc", b"fn other() {}"),
        ],
        None,
    );
    validate_package_sources(&valid).unwrap();
    for referenced_source in [
        r#"include_str!("/tmp/secret")"#,
        r#"#[path = "/tmp/mod.rs"] mod external;"#,
    ] {
        let invalid = package(
            &[
                ("flow.rs", br#"include!("hidden.inc");"#),
                ("hidden.inc", referenced_source.as_bytes()),
            ],
            None,
        );
        assert!(validate_package_sources(&invalid).is_err());
    }
}

#[test]
fn includes_are_relative_to_the_physical_file_inside_inline_modules() {
    let snapshot = package(
        &[
            ("flow.rs", b"mod outer;"),
            ("outer.rs", br#"mod inner { include!("fragment.inc"); }"#),
            (
                "fragment.inc",
                br#"const A: &str = include_str!("asset.txt");"#,
            ),
            ("asset.txt", b"frozen"),
        ],
        None,
    );
    validate_package_sources(&snapshot).unwrap();
}

#[test]
fn rejects_cycles_non_utf8_code_and_tampered_frozen_bytes() {
    let cycle = package(
        &[
            ("flow.rs", br#"include!("cycle.inc");"#),
            ("cycle.inc", br#"include!("flow.rs");"#),
        ],
        None,
    );
    assert!(format!("{:#}", validate_package_sources(&cycle).unwrap_err()).contains("cyclic"));
    let non_utf8 = package(
        &[("flow.rs", b"fn flow() {}"), ("secondary.rs", &[255])],
        None,
    );
    assert!(format!("{:#}", validate_package_sources(&non_utf8).unwrap_err()).contains("UTF-8"));
    let mut tampered = source_package("fn valid() {}");
    tampered
        .packages
        .get_mut(&tampered.root)
        .unwrap()
        .files
        .insert("secondary.rs".into(), b"changed".to_vec());
    assert!(validate_package_sources(&tampered).is_err());
}

#[test]
fn rejects_module_references_even_when_an_unrelated_declared_asset_has_the_same_name() {
    // A file declared under a different module directory must not satisfy the
    // declaration. Searching by basename would silently validate an external read.
    let snapshot = package(
        &[
            ("flow.rs", b"mod inline { mod missing; }"),
            ("missing.rs", b""),
        ],
        None,
    );
    assert!(validate_package_sources(&snapshot).is_err());
}

#[test]
fn filesystem_macro_names_remain_available_for_ordinary_rust_identifiers() {
    let snapshot = source_package(
        r#"
        struct Options { include: bool, include_str: String, include_bytes: Vec<u8>, r#mod: bool, r#use: bool }
        fn include() {} fn include_str() {} fn include_bytes() {}
        fn ordinary() { let include = true; let _ = include as usize; }
        use std::{include_str};
    "#,
    );
    validate_package_sources(&snapshot).unwrap();
}
