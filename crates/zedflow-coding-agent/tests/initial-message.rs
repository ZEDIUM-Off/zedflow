use zedflow_coding_agent::{Args, build_initial_message};
#[test]
fn initial_message_combines_cli_messages() {
    let mut args = Args {
        messages: vec!["one".into(), "two".into()],
        ..Args::default()
    };
    assert_eq!(
        build_initial_message(&mut args, Some("file "), &[], Some("stdin ")).initial_message,
        Some("stdin file one".into())
    );
    assert_eq!(args.messages, ["two"]);
}
