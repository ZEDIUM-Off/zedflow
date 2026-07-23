use serde_json::json;
use zedflow_coding_agent::{
    cli::{Mode, build_initial_message, parse_args},
    modes::{decode_json_lines, render_text_response, serialize_json_line},
};

#[test]
fn cli_preserves_prompt_file_and_unknown_flag_semantics() {
    let argv = [
        "--print",
        "hello",
        "@notes.md",
        "--plugin=on",
        "--thinking",
        "high",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let mut parsed = parse_args(&argv);
    assert_eq!(parsed.mode, None);
    assert!(parsed.print);
    assert_eq!(parsed.file_args, ["notes.md"]);
    assert_eq!(
        parsed.unknown_flags[0],
        ("plugin".into(), Some("on".into()))
    );
    assert_eq!(parsed.thinking, Some(zedflow_agent::ThinkingLevel::High));

    let prompt = build_initial_message(&mut parsed, Some("file\n"), &[], Some("stdin\n"));
    assert_eq!(
        prompt.initial_message.as_deref(),
        Some("stdin\nfile\nhello")
    );
    assert!(parsed.messages.is_empty());
}

#[test]
fn jsonl_is_lf_framed_and_text_mode_extracts_text_blocks() {
    let line = serialize_json_line(&json!({"text": "a\u{2028}b"})).unwrap();
    assert!(line.ends_with('\n'));
    assert_eq!(decode_json_lines(&line).unwrap().len(), 1);
    assert_eq!(
        render_text_response(
            &json!({"content": [{"type":"thinking","thinking":"x"},{"type":"text","text":"done"}]})
        ),
        Some("done".into())
    );
    assert_eq!(parse_mode(&["--mode", "rpc"]), Mode::Rpc);
}

fn parse_mode(args: &[&str]) -> Mode {
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    parse_args(&args).mode.unwrap()
}
