use zf_flows::bridge_source::{parse, remap_flow_imports};

fn assert_only_import_semantics_changed(source: &str, actual: &str, old: &str, new: &str) {
    let mut expected = parse(source).expect("valid original bridge");
    for import in expected.imports.values_mut() {
        if import.flow == old {
            new.clone_into(&mut import.flow);
        }
    }
    assert_eq!(
        serde_json::to_value(expected).unwrap(),
        serde_json::to_value(parse(actual).expect("valid remapped bridge")).unwrap()
    );
}

const SOURCE: &str = r#"// @zedflow-bridge 1
use zf_flows::composition::*;

// old stays here, including fake syntax: .import("fake", "old")
pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        .require("old")
        .import("a", /* old: keep this comment */ "old")
        .reuse("b", "old", "old")
        .import("old", "old-suffix")
        .connect("old", Connection::new(Endpoint::new("old", "old"), Endpoint::new("b", "old"), RouteMode::CallAwait, InvocationKind::Tool).tool("old").when(serde_json::json!({"old": "old"})))
        .bind("old", Endpoint::new("old", "old"), Endpoint::new("b", "old"), DataPermissions::read_only())
}
// old also stays at EOF
"#;

#[test]
fn replaces_only_import_flow_literals_preserving_all_other_bytes_and_semantics() {
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace(
            "use zf_flows::composition::*;",
            "use zedflow_daemon::harness::composition::*;",
        ),
    ] {
        let expected = source
            .replace(
                "/* old: keep this comment */ \"old\"",
                "/* old: keep this comment */ \"new\"",
            )
            .replace(".reuse(\"b\", \"old\",", ".reuse(\"b\", \"new\",");
        let actual = remap_flow_imports(&source, "old", "new").unwrap();
        assert_eq!(actual, expected);
        assert_only_import_semantics_changed(&source, &actual, "old", "new");
    }
}

#[test]
fn handles_unicode_raw_escaped_multiline_literals_and_crlf_without_offset_drift() {
    let source = r###"// @zedflow-bridge 1
use zf_flows::composition::*;
pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        /* é 🦀 before span */ .import("a", r##"clé
ancienne"##)
        .import("b", "cl\u{e9}\nancienne")
        .reuse("c", "clé
ancienne", "clé\nancienne") // unchanged instance
        .import("d", "cl\
            é\nancienne")
}
"###;
    let expected = r###"// @zedflow-bridge 1
use zf_flows::composition::*;
pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        /* é 🦀 before span */ .import("a", "next")
        .import("b", "next")
        .reuse("c", "next", "clé\nancienne") // unchanged instance
        .import("d", "next")
}
"###;
    for line_ending in ["\n", "\r\n"] {
        let source = source.replace('\n', line_ending);
        let actual = remap_flow_imports(&source, "clé\nancienne", "next").unwrap();
        assert_eq!(actual, expected.replace('\n', line_ending));
        assert_only_import_semantics_changed(&source, &actual, "clé\nancienne", "next");
    }
}

#[test]
fn rust_crlf_normalization_covers_raw_literals_without_changing_escaped_values() {
    let source = r##"// @zedflow-bridge 1
use zf_flows::composition::*;
pub fn bridge() -> BridgeDefinition {
    BridgeDefinition::new()
        .require(r"first
second")
        .import("a", r"first
second")
        .reuse("b", "first\r\nsecond", r"first
second")
        .connect("c", Connection::new(Endpoint::new(r"first
second", "out"), Endpoint::new("b", "in"), RouteMode::CallAwait, InvocationKind::Tool).tool(r"first
second").when(serde_json::json!({"message": r"first
second"})))
}
"##
    .replace('\n', "\r\n");
    let parsed = parse(&source).unwrap();
    assert_eq!(parsed.imports["a"].flow, "first\nsecond");
    assert_eq!(parsed.imports["b"].flow, "first\r\nsecond");
    assert_eq!(parsed.imports["b"].reuse.as_deref(), Some("first\nsecond"));
    assert!(parsed.requires.contains("first\nsecond"));
    assert_eq!(parsed.connections["c"].from.instance, "first\nsecond");
    assert_eq!(
        parsed.connections["c"].tool_name.as_deref(),
        Some("first\nsecond")
    );
    assert_eq!(
        parsed.connections["c"].condition.as_ref().unwrap()["message"],
        "first\nsecond"
    );

    let actual = remap_flow_imports(&source, "first\nsecond", "next").unwrap();
    assert_eq!(
        actual,
        source.replacen(
            ".import(\"a\", r\"first\r\nsecond\")",
            ".import(\"a\", \"next\")",
            1
        )
    );
    assert_only_import_semantics_changed(&source, &actual, "first\nsecond", "next");
    let escaped = remap_flow_imports(&source, "first\r\nsecond", "next").unwrap();
    assert_eq!(
        escaped,
        source.replace(
            ".reuse(\"b\", \"first\\r\\nsecond\",",
            ".reuse(\"b\", \"next\","
        )
    );
    assert_only_import_semantics_changed(&source, &escaped, "first\r\nsecond", "next");
}

#[test]
fn replacement_is_a_rust_literal_even_with_quotes_newlines_and_backslashes() {
    let new = "新\"\\\n\r\t\0🦀";
    let actual = remap_flow_imports(SOURCE, "old", new).unwrap();
    assert_only_import_semantics_changed(SOURCE, &actual, "old", new);
    assert!(actual.contains("/* old: keep this comment */"));
    assert!(actual.ends_with("// old also stays at EOF\n"));
}

#[test]
fn no_matching_import_or_same_key_preserves_source_exactly() {
    let source = SOURCE.replace('\n', "\r\n");
    for (old, new) in [("absent", "new"), ("old", "old")] {
        assert_eq!(remap_flow_imports(&source, old, new).unwrap(), source);
    }
    // A key appearing only in comments and conditions does not count as a hit.
    let source = SOURCE
        .replace("/* old: keep this comment */ \"old\"", "\"other\"")
        .replace(".reuse(\"b\", \"old\",", ".reuse(\"b\", \"other\",");
    assert_eq!(remap_flow_imports(&source, "old", "new").unwrap(), source);
}

#[test]
fn rejects_invalid_or_unrecognized_source_before_returning_even_without_hits() {
    let cases = [
        SOURCE.replace("\"old-suffix\"", "compute()"),
        SOURCE.replace("\"old-suffix\"", "concat!(\"old\", \"-suffix\")"),
        SOURCE.replace("\"old-suffix\"", "{ \"old-suffix\" }"),
        SOURCE.replace("\"old-suffix\"", "\"old-suffix\"custom_suffix"),
        SOURCE.replace("BridgeDefinition::new()", "return BridgeDefinition::new()"),
        SOURCE.replace("BridgeDefinition::new()", "{ BridgeDefinition::new() }"),
        SOURCE.replace("pub fn bridge()", "#[allow(dead_code)] pub fn bridge()"),
        format!("{SOURCE}\nfn ignored() {{ arbitrary_code(); }}"),
        SOURCE.replace(".import(\"a\",", ".unsupported(\"a\","),
        SOURCE.replace(".import(\"old\",", ".import(\"a\","),
        SOURCE.replace("// @zedflow-bridge 1", "// invalid header"),
        SOURCE.replace("pub fn bridge()", "pub fn bridge("),
    ];
    for source in cases {
        for (old, new) in [("old", "new"), ("absent", "new"), ("old", "old")] {
            assert!(remap_flow_imports(&source, old, new).is_err(), "{source}");
        }
    }
}

#[test]
fn rejects_remapping_that_exceeds_the_source_size_limit() {
    let oversized_key = "x".repeat(1024 * 1024);
    let diagnostics = remap_flow_imports(SOURCE, "old", &oversized_key).unwrap_err();
    assert!(format!("{diagnostics:?}").contains("exceeds 1 MiB"));
}
