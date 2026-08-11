use zedflow_coding_agent::modes::rpc::{JsonlReader, serialize_json_line};

#[test]
fn jsonl_frames_only_lf_and_preserves_unicode_separators() {
    let line = serialize_json_line(&serde_json::json!({"text":"a\u{2028}b\u{2029}c"}));
    let mut reader = JsonlReader::new();

    assert_eq!(
        reader.push(line.as_bytes()),
        vec![line.trim_end().to_owned()]
    );
    assert_eq!(
        JsonlReader::parse::<serde_json::Value>(line.trim()).unwrap(),
        serde_json::json!({"text":"a\u{2028}b\u{2029}c"})
    );
}

#[test]
fn jsonl_accepts_crlf_and_emits_an_unterminated_final_record() {
    let mut reader = JsonlReader::new();

    assert_eq!(reader.push(b"{\"a\":1}\r\n{\"b\":2}"), vec![r#"{"a":1}"#]);
    assert_eq!(reader.finish().as_deref(), Some(r#"{"b":2}"#));
}
