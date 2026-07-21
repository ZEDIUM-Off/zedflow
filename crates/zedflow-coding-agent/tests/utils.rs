use serde_yaml::Value;
use zedflow_coding_agent::utils::{
    ansi::strip_ansi,
    frontmatter::parse_frontmatter,
    html::{decode_html_entity, decode_html_entity_at},
    json::strip_json_comments,
    mime::detect_supported_image_mime_type,
    pi_user_agent::get_pi_user_agent,
};

#[test]
fn ports_deterministic_text_utilities() {
    assert_eq!(
        strip_ansi("a\x1b[31mred\x1b[0m\x1b]8;;https://example.com\x07link\x1b]8;;\x07z"),
        "aredlinkz"
    );
    assert_eq!(strip_ansi("\x1bcdone"), "done");

    let parsed = parse_frontmatter(
        "---\r\nname: test\r\ndescription: |\r\n  one\r\n  two\r\n---\r\n\r\nBody\r\n",
    )
    .unwrap();
    assert_eq!(parsed.frontmatter[Value::from("name")], Value::from("test"));
    assert_eq!(
        parsed.frontmatter[Value::from("description")],
        Value::from("one\ntwo\n")
    );
    assert_eq!(parsed.body, "Body");
    assert!(parse_frontmatter("---\nfoo: [bar\n---\nBody").is_err());

    assert_eq!(decode_html_entity("#x1F642").as_deref(), Some("🙂"));
    assert_eq!(decode_html_entity_at("x&amp;y", 1).unwrap().length, 5);
    assert_eq!(
        strip_json_comments("{\"url\":\"//🙂\",// c\n\"a\":[1,],}"),
        "{\"url\":\"//🙂\",\n\"a\":[1]}"
    );

    assert!(get_pi_user_agent("1.2.3").starts_with("pi/1.2.3 ("));
}

#[tokio::test]
async fn sleep_respects_abort_signal() {
    use zedflow_ai::utils::abort_signals::AbortController;
    use zedflow_coding_agent::utils::sleep::sleep;

    let controller = AbortController::new();
    controller.abort();
    assert_eq!(sleep(1, Some(&controller.signal())).await, Err("Aborted"));
    assert_eq!(sleep(0, None).await, Ok(()));
}

#[test]
fn detects_supported_image_types_and_rejections() {
    assert_eq!(
        detect_supported_image_mime_type(&[0xff, 0xd8, 0xff, 0xe0]),
        Some("image/jpeg")
    );
    assert_eq!(
        detect_supported_image_mime_type(&[0xff, 0xd8, 0xff, 0xf7]),
        None
    );
    assert_eq!(
        detect_supported_image_mime_type(b"GIF89a"),
        Some("image/gif")
    );
    assert_eq!(
        detect_supported_image_mime_type(b"RIFFxxxxWEBP"),
        Some("image/webp")
    );

    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&[0; 17]);
    png.extend_from_slice(b"\0\0\0\0IDAT");
    assert_eq!(detect_supported_image_mime_type(&png), Some("image/png"));
    png[37..41].copy_from_slice(b"acTL");
    assert_eq!(detect_supported_image_mime_type(&png), None);
}
